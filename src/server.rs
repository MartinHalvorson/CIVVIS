//! Zero-dependency local HTTP server for the human-vs-AI browser GUI.
//! Endpoints: GET / (page), GET /state, GET /machine-metrics, GET /save, GET /rules, GET /pedia,
//! POST /action, POST /step, POST /autoplay, POST /view,
//! POST /spectator-status, POST /next-game-settings, POST /new,
//! POST /supervisor-new.
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::ai::{AdvancedAi, Ai, BasicAi};
use crate::civ6;
use crate::game::{
    Action, Game, GameOptions, LeaderPool, PlayOnMode, VictoryConditions, CIV6_LEADER_POOL,
};
use crate::leader_roster;
use crate::obs::{observation, observation_player_view, observation_spectator};
use crate::name::Name;
use crate::rules::Rules;
use crate::setup::{
    future_era_from_id, future_era_id, start_era_from_id, start_era_id, turn_structure_id,
    BaseRuleset, FutureEra, GameSpeed, MapPoles, MapScript, MapSize, MapTopology, TurnStructure,
    BASE_RULESETS, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES, FUTURE_ERAS, MAP_POLES,
    MAP_TOPOLOGIES, START_ERAS,
};
use crate::Pos;

/// The published browser build's request router, which answers these same
/// endpoints inside the page instead of over a socket. A child module so it
/// can reach the private helpers below rather than widening them; `cfg`-gated
/// so no native build compiles it.
#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
pub mod wasm;

/// Which runtime is answering, so a long-lived page can notice it is talking
/// to a different one than it booted against and reload.
///
/// A native build answers with its process. `wasm32-unknown-unknown` has no
/// process to ask about — `std::process::id()` panics outright there — and a
/// browser tab is never handed off to a successor runtime mid-game, so the
/// published build is always the same one identity.
fn process_identity() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::id()
    }
}

#[cfg(not(target_arch = "wasm32"))]
static LAUNCHED_COMMIT: OnceLock<Option<String>> = OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static LAUNCHED_COMMIT_TIME: OnceLock<Option<String>> = OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static LAUNCHED_BUILT_AT: OnceLock<Option<String>> = OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static LAUNCHED_ARTIFACT_BYTES: OnceLock<Option<u64>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn promoted_binary_commit(name: &str) -> Option<String> {
    let commit = name.strip_prefix("civvis-")?;
    (commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| commit.to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
fn launched_commit() -> Option<String> {
    std::env::var("CIVVIS_COMMIT")
        .ok()
        .filter(|commit| !commit.is_empty())
        .or_else(|| {
            let executable = std::env::current_exe().ok()?;
            let name = executable.file_name()?.to_str()?;
            promoted_binary_commit(name)
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn launched_commit_time() -> Option<String> {
    std::env::var("CIVVIS_COMMIT_TIME")
        .ok()
        .filter(|commit_time| !commit_time.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn launched_built_at() -> Option<String> {
    std::env::var("CIVVIS_BUILT_AT")
        .ok()
        .filter(|built_at| !built_at.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn launched_artifact_bytes() -> Option<u64> {
    std::env::current_exe()
        .ok()?
        .metadata()
        .ok()
        .map(|metadata| metadata.len())
}

/// The revision of the code a supervisor selected for this process.
///
/// This deliberately reads the launch environment or promoted executable name
/// instead of `option_env!`: a compile-time revision would force a complete
/// optimized rebuild even when the source code itself has not changed.
/// The revision a supervisor selected, or `None` for an unstamped build.
///
/// ⚠ Deliberately NOT `runtime_commit("unknown")`: a run log that says
/// `"revision": "unknown"` reads like a failed lookup, while `null` says
/// plainly that nobody stamped this build. The identity that always reports is
/// the treatment list emitted beside it — see `civvis_orders`'s genome line.
pub fn runtime_commit_or_none() -> Option<String> {
    let commit = runtime_commit("");
    (!commit.is_empty()).then_some(commit)
}

pub(crate) fn runtime_commit(fallback: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        option_env!("CIVVIS_COMMIT")
            .filter(|commit| !commit.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        LAUNCHED_COMMIT
            .get_or_init(launched_commit)
            .clone()
            .unwrap_or_else(|| fallback.to_owned())
    }
}

/// When the selected revision was committed, as an ISO-8601 timestamp.
///
/// The desktop launcher supplies this beside `CIVVIS_COMMIT`; a published
/// browser build bakes both into its pinned module. An unstamped development
/// build returns `None` rather than presenting its compile time as source age.
pub(crate) fn runtime_commit_time() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        option_env!("CIVVIS_COMMIT_TIME")
            .filter(|commit_time| !commit_time.is_empty())
            .map(str::to_owned)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        LAUNCHED_COMMIT_TIME
            .get_or_init(launched_commit_time)
            .clone()
    }
}

/// When this exact artifact was built, as an ISO-8601 timestamp.
///
/// This stays separate from the revision's commit time: rebuilding current
/// source makes a fresh artifact, but does not rewrite Git history.
pub(crate) fn runtime_built_at() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        option_env!("CIVVIS_BUILT_AT")
            .filter(|built_at| !built_at.is_empty())
            .map(str::to_owned)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        LAUNCHED_BUILT_AT.get_or_init(launched_built_at).clone()
    }
}

/// Size of the exact artifact serving this page, in bytes.
///
/// A native process can inspect its own executable. The browser WASM module
/// has no filesystem path, so the published shim supplies its optimized byte
/// count from the matching lane manifest after publication.
pub(crate) fn runtime_artifact_bytes() -> Option<u64> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        *LAUNCHED_ARTIFACT_BYTES.get_or_init(launched_artifact_bytes)
    }
}

/// The kind of artifact whose size is reported beside the build ages.
pub(crate) const fn runtime_artifact_kind() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "WASM"
    } else {
        "native"
    }
}

/// Optional host-wide context for the display panel. The viewer's primary
/// performance readout is measured in the browser; these native-only values
/// deliberately expose percentages rather than pretending to be an app
/// memory or CPU inventory.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MachineMetrics {
    cpu_percent: Option<f64>,
    memory_percent: Option<f64>,
}

#[cfg(not(target_arch = "wasm32"))]
fn bounded_percent(value: f64) -> Option<f64> {
    value.is_finite().then(|| value.clamp(0.0, 100.0))
}

fn machine_metrics_value(metrics: MachineMetrics) -> Value {
    // One decimal place is precise enough for a display choice while avoiding
    // a falsely exact-looking value from a short host sample.
    let rounded = |value: Option<f64>| value.map(|value| (value * 10.0).round() / 10.0);
    json!({
        "cpu_percent": rounded(metrics.cpu_percent),
        "memory_percent": rounded(metrics.memory_percent),
    })
}

fn machine_metrics_json() -> Value {
    #[cfg(not(target_arch = "wasm32"))]
    let metrics = sampled_machine_metrics();
    #[cfg(target_arch = "wasm32")]
    let metrics = MachineMetrics::default();
    machine_metrics_value(metrics)
}

// A metrics request is cheap from the browser's point of view, but sampling a
// host can require a platform utility.  Reuse a short-lived answer for every
// open viewer so the load gauge cannot become the load it reports.
#[cfg(not(target_arch = "wasm32"))]
const MACHINE_METRICS_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct MachineMetricsSnapshot {
    sampled_at: Instant,
    metrics: MachineMetrics,
}

#[cfg(not(target_arch = "wasm32"))]
static MACHINE_METRICS_CACHE: OnceLock<Mutex<Option<MachineMetricsSnapshot>>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn sampled_machine_metrics() -> MachineMetrics {
    let cache = MACHINE_METRICS_CACHE.get_or_init(|| Mutex::new(None));
    let now = Instant::now();
    let mut held = cache.lock().unwrap();
    if let Some(snapshot) = *held {
        if now.duration_since(snapshot.sampled_at) < MACHINE_METRICS_SAMPLE_INTERVAL {
            return snapshot.metrics;
        }
    }
    let metrics = MachineMetrics {
        cpu_percent: host_cpu_percent(),
        memory_percent: host_memory_percent(),
    };
    *held = Some(MachineMetricsSnapshot {
        sampled_at: now,
        metrics,
    });
    metrics
}

#[cfg(target_os = "linux")]
fn linux_cpu_ticks(report: &str) -> Option<(u64, u64)> {
    let values: Vec<u64> = report
        .lines()
        .find(|line| line.starts_with("cpu "))?
        .split_whitespace()
        .skip(1)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let idle = *values.get(3)? + values.get(4).copied().unwrap_or(0);
    Some((values.iter().sum(), idle))
}

#[cfg(target_os = "linux")]
fn cpu_percent_from_ticks(previous: (u64, u64), current: (u64, u64)) -> Option<f64> {
    let total = current.0.checked_sub(previous.0)?;
    let idle = current.1.checked_sub(previous.1)?;
    (total > 0 && idle <= total).then(|| 100.0 * (total - idle) as f64 / total as f64)
}

#[cfg(target_os = "linux")]
fn host_cpu_percent() -> Option<f64> {
    static PREVIOUS: OnceLock<Mutex<Option<(u64, u64)>>> = OnceLock::new();
    let current = linux_cpu_ticks(&std::fs::read_to_string("/proc/stat").ok()?)?;
    let mut held = PREVIOUS.get_or_init(|| Mutex::new(None)).lock().unwrap();
    let measured = (*held).and_then(|previous| cpu_percent_from_ticks(previous, current));
    *held = Some(current);
    measured.and_then(bounded_percent)
}

#[cfg(target_os = "macos")]
fn macos_top_cpu_percent(report: &str) -> Option<f64> {
    let idle = report.lines().rev().find_map(|line| {
        let fragment = line.split(',').find(|part| part.contains("% idle"))?;
        fragment.split('%').next()?.trim().parse::<f64>().ok()
    })?;
    bounded_percent(100.0 - idle)
}

#[cfg(target_os = "macos")]
fn host_cpu_percent() -> Option<f64> {
    let output = Command::new("top").args(["-l", "1", "-n", "0"]).output().ok()?;
    macos_top_cpu_percent(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "linux", target_os = "macos"))
))]
fn host_cpu_percent() -> Option<f64> {
    None
}

#[cfg(target_os = "linux")]
fn memory_percent_from_meminfo(report: &str) -> Option<f64> {
    let mut total = None;
    let mut available = None;
    for line in report.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let Some(amount) = value
            .split_whitespace()
            .next()
            .and_then(|amount| amount.parse::<u64>().ok())
        else {
            continue;
        };
        match name {
            "MemTotal" => total = Some(amount),
            "MemAvailable" => available = Some(amount),
            _ => {}
        }
    }
    let total = total?;
    let available = available?;
    (total > 0 && available <= total)
        .then(|| 100.0 * (total - available) as f64 / total as f64)
}

#[cfg(target_os = "linux")]
fn host_memory_percent() -> Option<f64> {
    memory_percent_from_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
        .and_then(bounded_percent)
}

#[cfg(target_os = "macos")]
fn macos_physical_memory_bytes() -> Option<u64> {
    static TOTAL: OnceLock<Option<u64>> = OnceLock::new();
    TOTAL
        .get_or_init(|| {
            let output = Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
            String::from_utf8_lossy(&output.stdout).trim().parse().ok()
        })
        .to_owned()
}

#[cfg(target_os = "macos")]
fn macos_vm_stat_pages(report: &str, name: &str) -> Option<u64> {
    report.lines().find_map(|line| {
        let line = line.trim_start();
        let value = line.strip_prefix(name)?.trim_start_matches(':').trim();
        value.trim_end_matches('.').parse().ok()
    })
}

#[cfg(target_os = "macos")]
fn macos_memory_percent(report: &str, total: u64) -> Option<f64> {
    let page_size = report.lines().find_map(|line| {
        line.split("page size of ")
            .nth(1)?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    let available_pages = ["Pages free", "Pages inactive", "Pages speculative"]
        .iter()
        .filter_map(|name| macos_vm_stat_pages(report, name))
        .sum::<u64>();
    let available = available_pages.saturating_mul(page_size).min(total);
    (total > 0).then(|| 100.0 * (total - available) as f64 / total as f64)
}

#[cfg(target_os = "macos")]
fn host_memory_percent() -> Option<f64> {
    let total = macos_physical_memory_bytes()?;
    let output = Command::new("vm_stat").output().ok()?;
    macos_memory_percent(&String::from_utf8_lossy(&output.stdout), total).and_then(bounded_percent)
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "linux", target_os = "macos"))
))]
fn host_memory_percent() -> Option<f64> {
    None
}

const EMBEDDED_INDEX_HTML: &str = include_str!("../web/index.html");
const EMBEDDED_APP_JS: &str = include_str!("../web/assets/app.js");
/// The whole page source — the document plus its one external script — as a
/// single searchable string. The server ships the two parts at `/` and
/// `/assets/app.js`; the source-contract tests assert against this combined
/// source, which is the same contract they checked when the script block was
/// inline. (The split exists because the script was a 1.35 MB block inside
/// `web/index.html`, making that file the repository's largest history payer
/// at ~1 MB of pack growth per edit.)
static EMBEDDED_INDEX: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let mut page = String::with_capacity(EMBEDDED_INDEX_HTML.len() + EMBEDDED_APP_JS.len());
    page.push_str(EMBEDDED_INDEX_HTML);
    page.push_str(EMBEDDED_APP_JS);
    page
});
const EMBEDDED_FEATURE_ATLAS: &[u8] = include_bytes!("../web/assets/feature-atlas.png");
const EMBEDDED_ENVIRONMENT_FEATURE_ATLAS: &[u8] =
    include_bytes!("../web/assets/environment-feature-atlas.png");
const EMBEDDED_HIDDEN_MAP_MONSTERS: &[u8] =
    include_bytes!("../web/assets/hidden-map-monsters.png");
const EMBEDDED_CIV6_UNIT_FLAGS: &[u8] =
    include_bytes!("../web/assets/civ6-unit-flags.png");

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
    /// Which published game's rules the world is played by — the first thing
    /// the lobby asks, because it decides what every later answer means.
    pub base_ruleset: BaseRuleset,
    /// How far into history the world opens, as an era of
    /// [`crate::rules::ERA_NAMES`]. Only a rung the rules have a tree for ever
    /// reaches here; see [`new_game_params`].
    pub start_era: usize,
    /// Which rules the far end of the game is played by. Only an era somebody
    /// has built ever reaches here; see [`new_game_params`].
    pub future_era: FutureEra,
    /// Whether seats act sequentially or plan against a shared snapshot.
    /// Carried for provenance and faithful restarts; the interactive server
    /// itself only plays sequential games, and refuses the alternative at its
    /// entry points rather than quietly stepping a simultaneous world one
    /// seat at a time.
    pub turn_structure: TurnStructure,
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
    /// Roster used for every major seat the setup did not name explicitly.
    pub leader_pool: LeaderPool,
    /// Civilizations for the leading major seats, in seat order — seat 0 is
    /// the person's own. Empty is the stock roster; see `Game::seat_civs`.
    pub civs: Vec<String>,
    /// A lifecycle supervisor, rather than the browser countdown, owns the
    /// transition after a completed spectator game.
    pub supervised: bool,
    /// League directory to seat major players from (`civvis play --league`):
    /// each civ samples its top three live-eligible strategies with 3:2:1 rank
    /// weights, and the HUD shows per-player Elo. `None` still annotates Elo
    /// when a `league/` dir exists, because the default fleet below IS the
    /// league's "advanced" entrant — but the AIs themselves are unchanged.
    pub league_dir: Option<String>,
    /// Rate the finished game into `league_dir` (`--league-record`). Off by
    /// default because the shipped `data/league` roster is a committed
    /// snapshot: a run that seats from it must not rewrite it. Point this at
    /// a runtime copy and the table moves with every game played.
    pub league_record: bool,
    /// Optional match-machine coverage target. When set, this unretired
    /// strategy owns seat zero for the game; the other seats keep the normal
    /// rank-weighted exhibition selection. It is deliberately separate from
    /// world settings so rotating targets cannot trigger a spectator restart
    /// or alter tournament rules.
    pub force_strategy: Option<String>,
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
    /// Setup selected while this world is running. It is inert until the next
    /// automatic or explicitly requested simulation boundary.
    /// Setup for the next world carried in from launch flags by a resume.
    /// The live queue belongs to `Shared`; this only hands the resumed value
    /// across at construction, and is taken exactly once.
    resumed_next_game_params: Option<Params>,
    /// League roster used to label seats with player handles and elo (and,
    /// with `--league`, to choose who plays each civ).
    league: Option<crate::league::League>,
    /// Per-seat index into `league.strategies` for rated major seats.
    seat_strategy: Vec<Option<usize>>,
    /// Whether the roster above actually *chose* who plays each civ. Without
    /// `--league` it did not: every major runs the default hierarchical agent
    /// and each seat points at the entrant the league rates that agent as, so
    /// the rating is the seat's own but the handle is one name repeated down
    /// the table. Such a seat keeps its generated per-seat name instead.
    seat_from_roster: bool,
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
    /// What every agent at this table decided, and why.
    ///
    /// One journal for the whole session, handed to each seat, so a turn is
    /// one ordered account rather than one log per civilization. It is live
    /// only in spectate mode: nobody is watching a headless tournament, and a
    /// silent journal costs a branch. A restored save keeps the record it
    /// accumulates from the turn it resumes — the reasoning behind a decision
    /// is not part of the game state and cannot be recovered from one.
    journal: crate::reasoning::Journal,
    /// What became of every plan a simultaneous game has committed so far.
    /// Sequential games never touch it. The census is the regime's health
    /// instrument — a rising drop rate is the first sign the mode is
    /// distorting play — so the spectator state carries it alongside the
    /// turn structure itself.
    simultaneous_census: crate::simultaneous::SimultaneousCensus,
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
    buildings: BTreeMap<(u32, Name), usize>,
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
    districts: BTreeSet<Name>,
    buildings: BTreeSet<Name>,
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
                        district: district.clone().to_string(),
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
                    buildings.insert((city.id, *building), city.owner);
                }
            }
            for wonder in city.wonders.keys() {
                wonders.insert(wonder.to_string(), city.owner);
            }
            combat_owners
                .entry(city.pos)
                .or_default()
                .insert(city.owner);
        }
        let military_units = game
            .units
            .values()
            .filter(|unit| game.rules.units[unit.kind].class == "military")
            .map(|unit| {
                combat_owners
                    .entry(unit.pos)
                    .or_default()
                    .insert(unit.owner);
                (unit.id, unit.owner)
            })
            .collect();
        let tree_era = |nodes: &BTreeSet<Name>, technology: bool| {
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

fn completed_districts(game: &Game) -> BTreeSet<Name> {
    game.cities
        .values()
        .flat_map(|city| city.districts.keys())
        .cloned()
        .collect()
}

fn completed_buildings(game: &Game) -> BTreeSet<Name> {
    game.cities
        .values()
        .flat_map(|city| city.buildings.iter())
        .filter(|building| {
            game.rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.buildable)
        })
        .cloned()
        .collect()
}

/// One decimal place, the precision every accumulating figure in the
/// observation is reported at. A progress bar cannot show more, and the extra
/// digits are pure bytes on a document that is already the bottleneck.
fn round_tenth(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
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
        if chronicle.districts.insert(Name::new(&district.district)) {
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
        if chronicle.buildings.insert(*building) {
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
    /// The active world's identity without taking the simulation lock. This
    /// keeps a successor probe from queuing behind an AI turn just to learn
    /// whether the supervised process has changed.
    pub current_seed: AtomicU64,
    pub session: Mutex<Session>,
    /// A restart the viewer asked for, and the settings the live world was
    /// started from, both kept off the simulation lock for exactly the reason
    /// `current_seed` is.
    ///
    /// Pressing Restart used to take `session`, so it queued behind whatever
    /// AI turn was in flight. On the live exhibition that is not a theoretical
    /// wait: a late turn on a 74x46 six-player world held the lock for over
    /// two minutes, `/state` and `/status` both timed out, and the page — which
    /// gives up at fifteen seconds — cleared its veil and flashed an error
    /// while the supervisor, whose only view of the request is that same
    /// `/state`, never learned a restart had been asked for at all. Neither
    /// mutex here is ever held across anything but a field read.
    supervisor_request: Mutex<Option<Value>>,
    live_params: Mutex<Params>,
    /// Setup the viewer has chosen for the next world. Here for the same
    /// reason the request above is: it is written by a `change` on the setup
    /// panel, and `startNewSimulation` waits for that write before it asks for
    /// the restart — so a settings control that queued behind an AI turn put
    /// the whole restart behind it too.
    next_game_params: Mutex<Option<Params>>,
    pub pace_ms: AtomicU64,
    /// How long a completed spectator game remains on its result screen.
    /// This is a viewer preference, not part of the simulated world: each
    /// browser persists and reasserts its choice when the supervisor starts a
    /// fresh process.
    pub between_game_countdown_ms: AtomicU64,
    pub paused: AtomicBool,
    pub restart_in: AtomicU64, // ms until auto-restart; u64::MAX = not pending
    /// Measured wall time of a full game turn, including pacing sleeps.
    pub turn_us: AtomicU64,
    /// The same turn with the sleeps taken out: what the unlimited pace costs.
    pub turn_compute_us: AtomicU64,
    /// Monotonic identity of the last spectator frame the stepper completed.
    ///
    /// `turn` alone is not enough once Blitz and slower paces publish after
    /// every player's turn: every seat in a round shares the same world turn.
    /// This counter advances only when a frame is actually published, so the
    /// unpaced Lightning loop can still play a whole round into one frame.
    frame_sequence: AtomicU64,
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
    /// A victory may be decided by a seat in the middle of a round, without
    /// advancing `turn`. That terminal state is still a distinct frame.
    finished: bool,
    /// A server-local, monotonic publication sequence. Several player-turn
    /// frames can share `turn`; skipped Lightning seat steps share this value.
    sequence: u64,
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
    /// Every turn `POST /autoplay` has simulated since this server started.
    /// The denominator `missed` is otherwise missing: see
    /// [`FrameDelivery::turns_simulated_without_a_frame`].
    autoplayed: u64,
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
            if attached && previous.seed == frame.seed && frame.sequence > previous.sequence {
                lost = frame.sequence - previous.sequence - 1;
                seat.missed += lost;
            }
        }
        seat.painted = Some(frame);
        self.missed += lost;
    }

    /// Turns a single request simulated that no page could ever have drawn.
    ///
    /// Everything above counts a *viewer's* gaps: a page says which turn it
    /// painted, and the turns between that and its previous acknowledgement
    /// were simulated behind its back. Auto-play never has that conversation.
    /// The browser asks for `n` turns over `POST /autoplay`, the engine plays
    /// all of them, and exactly one state comes back — the one after the last.
    /// The other `n - 1` were played, were never serialised, and reached no
    /// screen. Nobody missed them; there was never a frame to miss.
    ///
    /// So they are counted here, at the only place that knows: the response is
    /// one state, and `played` says how many turns went into it. `played - 1`
    /// is the shortfall exactly, and it is the difference between a promise
    /// somebody remembered to keep in the client and one this server can be
    /// held to. `/status` reported a clean `frames_missed: 0` through a run
    /// that dropped nine turns in ten, because the only thing it knew how to
    /// count was an acknowledgement auto-play does not send.
    ///
    /// The turns are totalled as well as the shortfall, for the reason
    /// `frames_painted` exists: no misses is not the same claim as no misses
    /// *out of something*. A run of two hundred clean turns and a game where
    /// auto-play was never pressed both report zero, and only the total tells
    /// them apart.
    fn turns_simulated_without_a_frame(&mut self, played: usize) {
        self.autoplayed += played as u64;
        self.missed += played.saturating_sub(1) as u64;
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

/// The result screen starts at ten seconds, with the exact choices offered in
/// the observer settings. Keeping this short, closed set prevents a stale
/// client or an arbitrary HTTP request from turning the result screen into an
/// unexpectedly long hold.
const DEFAULT_BETWEEN_GAME_COUNTDOWN_MS: u64 = 10_000;
const BETWEEN_GAME_COUNTDOWN_OPTIONS_MS: [u64; 4] = [0, 3_000, 5_000, 10_000];
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
/// The unlimited pace steps flat out for this long, then rests. The rest is
/// also when the single-threaded accept loop gets a real slot, so the page
/// keeps loading state while the stepper runs.
const UNLIMITED_WORK_SLICE_MS: u64 = 90;
/// The rest after each work slice: ten milliseconds in every hundred holds
/// Lightning near 90% of a core instead of pinning it outright.
const UNLIMITED_REST_MS: u64 = 10;
/// Blitz is the first watch pace where every player's completed turn becomes
/// its own frame. Faster custom paces retain Lightning's one-frame-per-round
/// throughput; every offered pace at or above this threshold is seat-by-seat.
const PLAYER_TURN_FRAME_MIN_PACE_MS: u64 = 500;
/// Minor civilizations and barbarians take a quarter of a major's slice.
const MINOR_SHARE: f64 = 0.25;

fn publishes_player_turn_frames(pace_ms: u64) -> bool {
    pace_ms >= PLAYER_TURN_FRAME_MIN_PACE_MS
}

fn spectator_frame(game: &Game, sequence: u64) -> SpectatorFrame {
    SpectatorFrame {
        seed: game.seed,
        turn: game.turn,
        finished: game.winner.is_some(),
        sequence,
    }
}

fn spectator_step_completes_frame(
    pace_ms: u64,
    turn_before: u32,
    finished_before: bool,
    game: &Game,
) -> bool {
    publishes_player_turn_frames(pace_ms)
        || game.turn != turn_before
        || game.winner.is_some() != finished_before
}

/// The result countdown can only use the four values the viewer can select.
fn valid_between_game_countdown_ms(value: u64) -> Option<u64> {
    BETWEEN_GAME_COUNTDOWN_OPTIONS_MS
        .contains(&value)
        .then_some(value)
}

/// Read the viewer-selected hold through one helper so both places that arm
/// and advance the countdown agree on the active value.
fn final_countdown_ms(sh: &Shared) -> u64 {
    sh.between_game_countdown_ms.load(Ordering::Relaxed)
}

/// Give an unrated AI seat a compact, deterministic handle. The first half of
/// the handle follows its published grand strategy; the second distinguishes
/// seats pursuing the same plan. A strategy reassessment may therefore rename
/// an unrated agent, which makes the player column describe what is actually
/// running now instead of preserving a stale opening label.
fn generated_ai_name(seed: u64, pid: usize, strategy: Option<&str>) -> String {
    const ROLES: [&str; 12] = [
        "Architect",
        "Pathfinder",
        "Steward",
        "Visionary",
        "Tactician",
        "Builder",
        "Navigator",
        "Marshal",
        "Sage",
        "Keeper",
        "Pioneer",
        "Planner",
    ];
    let prefixes = match strategy {
        Some("expansion") => ["Frontier", "Horizon", "Homestead", "Border"],
        Some("science") => ["Quantum", "Stellar", "Orbital", "Theory"],
        Some("culture") => ["Mosaic", "Lyric", "Gallery", "Festival"],
        Some("religion" | "religious") => ["Pilgrim", "Sacred", "Temple", "Oracle"],
        Some("diplomacy" | "diplomatic") => ["Concord", "Treaty", "Envoy", "Summit"],
        Some("conquest" | "domination") => ["Vanguard", "Iron", "Siege", "Legion"],
        Some("recovery") => ["Phoenix", "Bastion", "Rally", "Reserve"],
        _ => ["Adaptive", "Strategic", "Resolute", "Calculated"],
    };
    let seed_mix = (seed ^ (seed >> 32)) as usize;
    let prefix = prefixes[(pid + seed_mix) % prefixes.len()];
    let role = ROLES[(pid / prefixes.len() + seed_mix / prefixes.len()) % ROLES.len()];
    let cycle = pid / (prefixes.len() * ROLES.len());
    if cycle == 0 {
        format!("{prefix}{role}")
    } else {
        format!("{prefix}{role}{}", cycle + 1)
    }
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
                spectator_frame(
                    &session.game,
                    self.frame_sequence.load(Ordering::Relaxed),
                )
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

    /// Frames this server published that some viewer never drew, the newest
    /// exact frame one reported drawing, and how many pages are watching. No
    /// painted frame at all means nobody was watching, which is a different
    /// thing from a promise being kept. The fourth result is auto-play's own
    /// denominator: turns it simulated, so zero can be told apart from a game
    /// where nobody ever pressed it.
    fn frame_audit(&self) -> (u64, Option<SpectatorFrame>, usize, u64) {
        let mut delivery = self.frame_delivery.lock().unwrap();
        delivery.retire_departed(Instant::now());
        let missed = delivery.missed;
        let painted = delivery
            .seats
            .values()
            .filter_map(|seat| seat.painted)
            .max_by_key(|frame| frame.sequence);
        (missed, painted, delivery.seats.len(), delivery.autoplayed)
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
    /// Ask the supervisor to replace this process with a new simulation.
    ///
    /// Deliberately touches nothing the AI holds. The whole point of the
    /// control is to get out of the world that is running, so it cannot be the
    /// one request that waits on it: see the `supervisor_request` field.
    fn request_supervised_new_game(&self, request: &Value) -> Result<(), String> {
        let base = self.live_params.lock().unwrap().clone();
        if !base.supervised {
            return Err("fresh-code launches require the spectator supervisor".into());
        }
        // Bind the request to the world the page offered to replace. A result
        // countdown or an old tab can outlive that process, and the same port
        // then belongs to a healthy successor. Before this check, its delayed
        // POST was accepted by the successor and the supervisor killed a game
        // that had only run for a few turns. Both values are lock-free so the
        // restart control still works while an AI turn holds the session.
        let expected = request
            .get("replace_world")
            .ok_or_else(|| "replace_world is required".to_string())?;
        let expected_seed = expected["seed"]
            .as_u64()
            .ok_or_else(|| "replace_world.seed must be an integer".to_string())?;
        let expected_instance = expected["server_instance"]
            .as_u64()
            .ok_or_else(|| "replace_world.server_instance must be an integer".to_string())?;
        if expected_seed != self.current_seed.load(Ordering::Relaxed)
            || expected_instance != process_identity() as u64
        {
            return Err("the world changed before the restart began".into());
        }
        let restart_source = request["restart_source"].as_str().unwrap_or("unknown");
        if restart_source == "finale_countdown" {
            // A process/seed pair proves which world a callback belongs to,
            // not that the callback had a reason to replace it. The browser's
            // unattended human-game timer is the only non-user caller. Make
            // that intent explicit and verify its terminal condition here so
            // a bad current-world callback cannot retire a healthy exhibition.
            //
            // This exceptional path may take the simulation lock. A genuine
            // finale has no AI turn in flight, while manual Restart remains
            // lock-free so it can still escape a long or wedged turn.
            let session = self.session.lock().unwrap();
            let human_was_eliminated = !session.params.spectate
                && session
                    .game
                    .players
                    .get(0)
                    .is_some_and(|player| !player.alive);
            if session.game.seed != expected_seed
                || (session.game.winner.is_none() && !human_was_eliminated)
            {
                return Err("automatic restart requires a finished game".into());
            }
        }
        let mode = request["mode"]
            .as_str()
            .ok_or_else(|| "mode must be restart or fresh_code".to_string())?;
        if mode != "restart" && mode != "fresh_code" {
            return Err("mode must be restart or fresh_code".into());
        }

        let paused = request["paused"]
            .as_bool()
            .unwrap_or_else(|| self.paused.load(Ordering::Relaxed));
        let mut params = new_game_params(&base, request);
        params.spectate = true;
        *self.supervisor_request.lock().unwrap() = Some(json!({
            "mode": mode,
            "source": restart_source,
            "server_instance": process_identity(),
            "paused": paused,
            "settings": simulation_settings(&params),
        }));
        eprintln!(
            "[server] accepted supervised {mode} request from {restart_source} for instance {} seed {expected_seed}",
            process_identity()
        );
        // The world on screen is being replaced. Stop stepping it rather than
        // spending the handoff computing turns nobody will ever be shown.
        self.paused.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Queue setup controls for the next world without changing this one.
    fn stage_next_game_settings(&self, request: &Value) {
        let base = self.live_params.lock().unwrap().clone();
        *self.next_game_params.lock().unwrap() = Some(staged_next_game_params(&base, request));
    }

    /// What the next world will be started from, as the setup panel reads it.
    fn staged_next_game_settings(&self) -> Value {
        self.next_game_params
            .lock()
            .unwrap()
            .as_ref()
            .map(simulation_settings)
            .unwrap_or(Value::Null)
    }

    /// Hand the queue to the world that is about to consume it.
    fn take_next_game_params(&self) -> Option<Params> {
        self.next_game_params.lock().unwrap().take()
    }

    /// The request the supervisor is being asked to act on, if any.
    fn pending_new_game_request(&self) -> Option<Value> {
        self.supervisor_request.lock().unwrap().clone()
    }

    /// Adopt the settings a freshly started world runs under, so the next
    /// new-game request can be normalized against them without the lock.
    fn adopt_live_params(&self, params: &Params) {
        *self.live_params.lock().unwrap() = params.clone();
    }

    /// Whether any viewer still present is owed `frame`.
    ///
    /// The same question `wait_for_turn_frame` blocks on, asked once. The
    /// stepper uses it to re-confirm, under the frame gate, that the answer it
    /// waited for outside the gate is still true.
    fn frame_outstanding(&self, frame: SpectatorFrame) -> bool {
        self.frame_delivery
            .lock()
            .unwrap()
            .wait_remaining(frame, Instant::now())
            .is_some()
    }

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

/// Derive the next unattended-world seed without leaving JavaScript's exact
/// integer range.
///
/// Spectator pages return the seed in `world=` when acknowledging a painted
/// frame. JSON parses an integer above 2^53 - 1 as a rounded JavaScript
/// `Number`, so a full-width successor seed can never match the server's u64
/// and the opening frame remains gated. The low 53 bits of this full-period
/// LCG are themselves a full-period sequence, retaining deterministic variety
/// while keeping the browser/server identity round-trip exact.
const MAX_EXACT_JAVASCRIPT_INTEGER: u64 = (1_u64 << 53) - 1;

fn automatic_successor_seed(seed: u64) -> u64 {
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        & MAX_EXACT_JAVASCRIPT_INTEGER
}

impl Session {
    /// Seat AIs plus each seat's league identity. With a roster to seat from,
    /// each major gets a rank-weighted sample from the top three proven winners
    /// for this table size (`league::seat_by_civ_seeded`), with its exact
    /// leader/civilization placement rating breaking win-bound ties and repeats
    /// avoided while possible. Otherwise majors run the default hierarchical
    /// AI, which the league rates as its "advanced" entrant, so a loaded roster
    /// can still label those seats with an elo.
    ///
    /// A seat somebody is playing is never seated from the roster. Whoever is
    /// at the keyboard is their own player — `register_human_players` gives
    /// them a new one — and an entrant that had this seat handed to it would
    /// wear a person's game as its own result.
    fn ai_fleet(
        game: &Game,
        league: Option<&crate::league::League>,
        seat_from_roster: bool,
        force_strategy: Option<&str>,
    ) -> (Vec<Box<dyn Ai + Send>>, Vec<Option<usize>>) {
        let mut seat_strategy: Vec<Option<usize>> = vec![None; game.players.len()];
        if let Some(l) = league {
            let majors: Vec<usize> = game
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian && !game.is_human_seat(p.id))
                .map(|p| p.id)
                .collect();
            if seat_from_roster && !l.exhibition_active().is_empty() {
                let civs: Vec<String> =
                    majors.iter().map(|id| game.players[*id].civ.clone()).collect();
                let table_size = game
                    .players
                    .iter()
                    .filter(|player| !player.is_minor && !player.is_barbarian)
                    .count();
                for (id, pick) in majors.iter().zip(crate::league::seat_by_civ_seeded(
                    l,
                    &civs,
                    table_size,
                    game.seed,
                    3,
                )) {
                    seat_strategy[*id] = Some(pick);
                }
            } else if let Some(default_entrant) =
                l.strategies.iter().position(|s| s.name == "advanced")
            {
                for id in &majors {
                    seat_strategy[*id] = Some(default_entrant);
                }
            }
            // The match machine rotates every unretired strategy through a
            // dedicated seat. This includes an entrant marked `league_only`:
            // it is still a valid rated strategy, and an explicit coverage
            // request is the operator's admission decision for this game.
            // Keep all other seats on the ordinary sampler and swap rather
            // than duplicate the target when it was already selected.
            if let Some(name) = force_strategy {
                if let Some(target) = l.strategies.iter().position(|strategy| {
                    strategy.name == name && !strategy.retired && !strategy.human
                }) {
                    if let Some(seat) = majors.first().copied() {
                        if let Some(existing) = seat_strategy
                            .iter()
                            .position(|strategy| *strategy == Some(target))
                        {
                            seat_strategy.swap(seat, existing);
                        } else {
                            seat_strategy[seat] = Some(target);
                        }
                    }
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
            None => crate::league::shipped_league(),
        }
    }

    /// The roster named by `--league`, else a best-effort load purely for
    /// elo labels: the runtime `league/` this checkout records into, then the
    /// snapshot compiled into every build. Only the named roster is ever
    /// written to, so a labelled game still rates nothing — but it does say
    /// what the league already knows about the agent in each seat, which used
    /// to require passing `--league` to see at all.
    ///
    /// The last step is `league::shipped_league` rather than a read of
    /// `data/league`, because a directory is resolved against the working
    /// directory and a rating must not depend on where the binary was started
    /// from. Reading that path is what left every seat at a provisional 1500
    /// anywhere but a checkout root.
    fn load_params_league(params: &Params) -> (Option<crate::league::League>, bool) {
        match &params.league_dir {
            Some(dir) => (crate::league::load_league(dir), true),
            None => (
                crate::league::load_league("league").or_else(crate::league::shipped_league),
                false,
            ),
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
        let (league, seat_from_roster) = Self::load_params_league(&params);
        Self::new_with_league(params, league, seat_from_roster)
    }

    /// Construct a world against a roster supplied by its host.
    ///
    /// Native sessions normally load from `Params::league_dir`. A browser
    /// module has no filesystem, so the local desktop host hands it the same
    /// live roster as the native spectator and asks it to seat directly from
    /// that snapshot.
    fn new_with_league(
        params: Params,
        mut league: Option<crate::league::League>,
        seat_from_roster: bool,
    ) -> Session {
        // Seat 0 is the person at the keyboard, which is what decides who the
        // difficulty hands its bonuses to. A spectated game has nobody there.
        let human_seats = if params.spectate {
            BTreeSet::new()
        } else {
            BTreeSet::from([0usize])
        };
        let mut game = Game::new_with(GameOptions {
            base_ruleset: params.base_ruleset,
            start_era: params.start_era,
            future_era: params.future_era,
            turn_structure: params.turn_structure,
            map_script: params.map_script,
            map_topology: params.map_topology,
            map_poles: params.map_poles,
            difficulty: params.difficulty.clone(),
            speed: params.speed.clone(),
            human_seats,
            teams: params.teams.clone(),
            civs: params.civs.clone(),
            leader_pool: params.leader_pool,
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
        // The hierarchical agent is the stock major-civilization default when
        // no league strategy is seated. Minors/barbarians retain the cheaper
        // baseline because they do not need empire-level planning.
        let (mut ais, mut seat_strategy) = Self::ai_fleet(
            &game,
            league.as_ref(),
            seat_from_roster,
            params.force_strategy.as_deref(),
        );
        // Only a watched table records its reasoning. Everywhere else the
        // journal is off and every `think!` is one `Option` test.
        let journal = if params.spectate {
            crate::reasoning::Journal::recording()
        } else {
            crate::reasoning::Journal::default()
        };
        for ai in &mut ais {
            ai.attach_journal(journal.handle());
        }
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
            resumed_next_game_params: None,
            league,
            seat_strategy,
            seat_from_roster,
            roster,
            autoplay_strategy: None,
            last_autoplay_request: None,
            league_recorded: false,
            human_players,
            journal,
            simultaneous_census: crate::simultaneous::SimultaneousCensus::default(),
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
        params.base_ruleset = game.base_ruleset;
        params.start_era = game.start_era;
        params.future_era = game.future_era;
        params.turn_structure = game.turn_structure;
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
        params.leader_pool = game.leader_pool;
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
        let (mut ais, mut seat_strategy) = Self::ai_fleet(
            &game,
            league.as_ref(),
            seat_from_roster,
            params.force_strategy.as_deref(),
        );
        // A save carries the world, not what anyone was thinking while they
        // played it. The restored table starts a fresh record from the turn it
        // resumes.
        let journal = if params.spectate {
            crate::reasoning::Journal::recording()
        } else {
            crate::reasoning::Journal::default()
        };
        for ai in &mut ais {
            ai.attach_journal(journal.handle());
        }
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
            resumed_next_game_params: next_game_params,
            league,
            seat_strategy,
            seat_from_roster,
            roster,
            autoplay_strategy: None,
            last_autoplay_request: None,
            league_recorded,
            human_players,
            journal,
            simultaneous_census: crate::simultaneous::SimultaneousCensus::default(),
        }
    }

    fn set_view_player(&mut self, player: Option<usize>) -> Result<(), String> {
        if let Some(pid) = player {
            let Some(candidate) = self.game.players.get(pid) else {
                return Err(format!("unknown player {pid}"));
            };
            if candidate.is_minor || candidate.is_barbarian {
                return Err(format!("player {pid} is not a major civilization"));
            }
        }
        // Selecting either a civilization or the all-player Spectator heading
        // in the HUD is the handoff from an interactive match to AI-only
        // observation. Keep the current world intact; the already-created AI
        // fleet can take over every seat on the next spectator step.
        self.params.spectate = true;
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
                || expected_instance != process_identity() as u64
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

    /// Queue setup controls for the next world without changing this one.
    /// Take the setup a resume carried in, for `Shared` to hold from here on.
    fn take_resumed_next_game_params(&mut self) -> Option<Params> {
        self.resumed_next_game_params.take()
    }

    /// Start the next world, from whatever setup the viewer queued for it.
    ///
    /// The queue is passed in rather than read from here: it is written by a
    /// setup control that must never wait on the simulation, so it lives
    /// beside the session rather than inside it.
    fn start_automatic_next_game(&mut self, queued: Option<Params>) {
        let next_seed = automatic_successor_seed(self.params.seed);
        let mut params = queued.unwrap_or_else(|| self.params.clone());
        params.seed = next_seed;
        *self = Session::new(params);
    }

    /// An agent's plan as the spectator sees it. City ids mean nothing to a
    /// browser, so each one is resolved here into the name and owner the HUD
    /// can actually print.
    /// The reasoning recorded since the observer's `cursor`.
    ///
    /// This is a *delta*, and it has to be. `/state` is close to a megabyte on
    /// a standard map and the watching page fetches one per turn; sending the
    /// whole reasoning ring alongside it every time would add a fixed six
    /// thousand entries to a document that is already the bottleneck. A page
    /// asks with the last id it holds and is answered with what came after it,
    /// which on a normal turn is a few dozen lines.
    ///
    /// A page that names a cursor this log has never issued — a tab that
    /// survived into the next world — is answered with the whole window and
    /// told to discard what it holds.
    pub fn reasoning_json(&self, cursor: u64) -> Value {
        let delta = self.journal.since(cursor);
        // Watching *as* a civilization means seeing what that civilization can
        // see, and nobody can read a rival's mind. The redaction is done here,
        // on the wire, rather than by hiding rows in the browser — the same
        // rule the observation itself follows for an unmet civ.
        let seat = self.view_player;
        let thoughts: Vec<&crate::reasoning::Thought> = delta
            .thoughts
            .iter()
            .filter(|thought| seat.is_none_or(|pid| thought.player == pid))
            .collect();
        json!({
            "cursor": delta.cursor,
            "reset": delta.reset,
            // Reasoning the ring has already evicted, and civilization-turns
            // cut short by the per-turn budget. A log that is not the whole
            // story has to say so rather than read as a quiet game.
            "dropped": delta.dropped,
            "truncated_turns": delta.truncated_turns,
            "thoughts": thoughts.iter().map(|thought| {
                let mut value = json!({
                    "id": thought.id,
                    "turn": thought.turn,
                    "player": thought.player,
                    "topic": thought.topic.as_str(),
                    "level": thought.level.as_str(),
                    "headline": thought.headline,
                });
                // Two fields most thoughts do not carry. Omitting them rather
                // than sending empty ones is worth real bytes over a delta of
                // a few hundred lines a turn.
                if !thought.detail.is_empty() {
                    value["detail"] = json!(thought.detail);
                }
                if let Some((q, r)) = thought.focus {
                    value["pos"] = json!([q, r]);
                }
                value
            }).collect::<Vec<_>>(),
        })
    }

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
            "war": plan.war.as_ref().map(|war| json!({
                "enabled": war.enabled,
                "selective": war.selective,
                "rapid": war.rapid,
                "active": war.active,
                "phase": war.phase,
                "target_player": war.target_player,
                "objective_city": city(war.objective_city),
                "breakthrough_tech": war.breakthrough_tech,
                "assault_unit": war.assault_unit,
                "predecessor": war.predecessor,
                "breach_unit": war.breach_unit,
                "required_bodies": war.required_bodies,
                "ready_bodies": war.ready_bodies,
                "staged_bodies": war.staged_bodies,
                "breach_ready": war.breach_ready,
                "upgrade_gold_reserved": (war.upgrade_gold_reserved * 10.0).round() / 10.0,
                "appointed_turn": war.appointed_turn,
                "appointments": war.appointments,
                "breakthroughs": war.breakthroughs,
                "mobilizations": war.mobilizations,
                "declarations": war.declarations,
                "complete_package_declarations": war.complete_package_declarations,
                "objectives_captured": war.objectives_captured,
                "objectives_captured_within_ten": war.objectives_captured_within_ten,
                "aborts": war.aborts,
            })),
        })
    }

    /// Say who is playing each human seat: the handle this game registered
    /// for them and the rating they are defending. Somebody who has finished
    /// nothing still has a rating — the base every player starts from — so
    /// the seat carries 1500 marked `player_elo_provisional` rather than a
    /// blank, and their first result moves it up or down from there.
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
            // Their own row, found by the name they were registered under —
            // not `seat_entry`, which follows whichever agent is holding the
            // seat. Handing it to auto-play does not hand over your rating.
            let entry = self
                .league
                .as_ref()
                .and_then(|league| league.strategies.iter().find(|s| s.name == seat.name));
            let civ = &self.game.players[id].civ;
            let shown = crate::league::display_rating(entry, civ);
            player["player_elo"] = json!(shown.rating.round() as i64);
            player["player_elo_rd"] = json!(shown.rd.round() as i64);
            player["player_elo_civ"] = json!(shown.civ_specific);
            player["player_elo_provisional"] = json!(shown.provisional);
            player["player_games"] = json!(entry.map_or(0, |entry| entry.games));
        }
    }

    /// The league row backing a seat, if the league has one. `None` is the
    /// ordinary case for a game running without a roster, and is a seat the
    /// league has never heard of rather than a seat that cannot be rated:
    /// `league::display_rating` gives it the provisional base.
    fn seat_entry(&self, seat: usize) -> Option<&crate::league::Strategy> {
        let index = self.seat_strategy.get(seat).copied().flatten()?;
        self.league.as_ref()?.strategies.get(index)
    }

    /// Give every met major the rating it is defending and both of its odds of
    /// winning this game: the ones it sat down with, and the ones it holds now.
    ///
    /// Every major carries a rating, roster or no roster. Most games run
    /// without `--league`, and a column of dashes read as "this build cannot
    /// rate anyone" when the truth was only that nobody at this table has
    /// finished a rated game yet. An unknown player is a provisional 1500
    /// that the first result moves. A standing is public the way a chess
    /// opponent's is, so an *opponent* in an interactive game carries one
    /// too — subject to the same fog as everything else here: a civ you have
    /// not met is not annotated at all.
    ///
    /// The odds themselves are [`crate::odds`]. This is where the ratings live,
    /// which is why the model is fed from here: the seat's league number, plus
    /// its civilization's measured edge where that number is not already
    /// civ-specific, becomes the prior the difficulty setting and the board then
    /// correct. A game with no roster still gets both figures — every seat is
    /// then the same provisional 1500, so the start odds are the difficulty
    /// bargain and the size of the table, which is exactly what they should be.
    fn name_seat_ratings(&self, o: &mut Value) {
        let g = &self.game;
        let rating_of = |pid: usize| {
            crate::league::display_rating(self.seat_entry(pid), &g.players[pid].civ)
        };
        // The rating each seat brings to the table, before the world touches
        // it. A seat the league has never heard of is in here at the
        // provisional base rather than left out: leaving it out would make the
        // remaining shares add to one between themselves and quietly claim the
        // unrated seat cannot win.
        let odds = crate::odds::table(g, |pid| {
            let shown = rating_of(pid);
            let elo = if shown.civ_specific {
                // Already a measurement of this player as this civilization.
                shown.rating
            } else {
                // Otherwise the roster still knows something about the civ
                // they drew, even if this player has never played it.
                shown.rating
                    + self
                    .league
                    .as_ref()
                    .map_or(0.0, |league| {
                        crate::odds::civ_edge_elo(league, &g.players[pid].civ)
                    })
            };
            // The midpoint and the uncertainty are both part of a pregame
            // prediction. A one-game rating must not make the same promise as
            // a settled one merely because the two badges show the same Elo.
            crate::odds::PriorRating::new(elo, shown.rd)
        });
        let Some(players) = o["players"].as_array_mut() else {
            return;
        };
        for player in players {
            let Some(id) = player["id"].as_u64().map(|id| id as usize) else {
                continue;
            };
            if player["met"] == json!(false)
                || !g
                    .players
                    .get(id)
                    .is_some_and(|p| !p.is_minor && !p.is_barbarian)
            {
                continue;
            }
            let shown = rating_of(id);
            // A handle names a seat only when the roster picked that seat.
            // Otherwise the whole table is the same entrant and repeating its
            // name down every row would hide who is who; the generated
            // per-seat name from `name_ai_players` reads better and is just
            // as true.
            if let (true, Some(s)) = (self.seat_from_roster, self.seat_entry(id)) {
                // Stable rating identity, distinct from both the friendly
                // username and the controller's one-word tactical label.
                // Hosted browser games return this exact value when filing
                // the terminal result into the persistent league.
                if !s.human {
                    player["ai_player_strategy"] = json!(s.name);
                }
                player["ai_username"] = json!(s.username);
                player["ai_strat_label"] = json!(s.label());
            }
            player["ai_elo"] = json!(shown.rating.round() as i64);
            player["ai_elo_rd"] = json!(shown.rd.round() as i64);
            player["ai_elo_civ"] = json!(shown.civ_specific);
            player["ai_elo_provisional"] = json!(shown.provisional);
            if let Some(seat) = odds.get(&id) {
                // Preserve the model's positive probability exactly. Rounding
                // at the transport boundary can turn a living long shot into
                // numeric zero, which the client correctly reserves for a seat
                // that cannot win. The browser owns display rounding.
                player["odds_start"] = json!(seat.start);
                player["odds_now"] = json!(seat.now);
                // What made the number, so a dossier can say why rather than
                // asking the reader to trust a percentage.
                player["odds_prior_elo"] = json!(seat.prior_elo.round() as i64);
                player["odds_handicap_elo"] = json!(seat.handicap_elo.round() as i64);
                player["odds_standing_elo"] = json!(seat.standing_elo.round() as i64);
            }
        }
    }

    /// Name every major AI visible in this observation, including opponents
    /// in an interactive game. Human identity is written afterward and wins
    /// in the browser, while a seat handed over through Watch as naturally
    /// falls back to this agent name.
    fn name_ai_players(&self, o: &mut Value) {
        let Some(players) = o["players"].as_array_mut() else {
            return;
        };
        for player in players {
            let Some(id) = player["id"].as_u64().map(|id| id as usize) else {
                continue;
            };
            if player["met"] == json!(false)
                || !self
                    .game
                    .players
                    .get(id)
                    .is_some_and(|seat| !seat.is_minor && !seat.is_barbarian)
            {
                continue;
            }
            let strategy = self.ais.get(id).and_then(|ai| ai.strategy_label());
            player["ai_name"] = json!(generated_ai_name(self.game.seed, id, strategy));
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
            if let Some(players) = o["players"].as_array_mut() {
                for player in players {
                    let Some(id) = player["id"].as_u64().map(|id| id as usize) else {
                        continue;
                    };
                    // A perspective the observation has already withheld does
                    // not get its plan pinned back on here. Only the
                    // omniscient view annotates everyone.
                    if player["met"] == json!(false) {
                        continue;
                    }
                    let strategy = self.ais.get(id).and_then(|ai| ai.strategy_label());
                    if let Some(strategy) = strategy {
                        player["ai_strategy"] = json!(strategy);
                    }
                    // The expanded HUD card explains a civilization's whole
                    // medium-term plan, not just its one-word label, so the
                    // spectator frame carries the agent's own read-out.
                    if let Some(plan) = self.ais.get(id).and_then(|ai| ai.plan_report()) {
                        player["ai_plan"] = self.plan_json(&plan);
                    }
                    // What this civilization is actually spending its science
                    // and culture on, for the AI strategy dossier. The
                    // observation only ever carries the *observed* seat's, in
                    // `me`, and above the world there is no observed seat.
                    //
                    // Omniscient view only. Watching as one civilization means
                    // seeing what that civilization sees, and a rival's
                    // laboratory is not on that list — the same rule
                    // `reasoning_json` applies to a rival's thoughts. That seat
                    // reads its own out of `me` as it always has.
                    if self.view_player.is_none() {
                        if let Some(seat) = g.players.get(id) {
                            if !seat.is_minor && !seat.is_barbarian {
                                player["research"] = json!(seat.research);
                                player["research_progress"] =
                                    json!(round_tenth(seat.research_progress));
                                player["civic"] = json!(seat.civic);
                                player["civic_progress"] = json!(round_tenth(seat.civic_progress));
                                // Only whether the *current* study is boosted.
                                // A late-game empire's whole boosted set is
                                // dozens of strings per seat on a document that
                                // is already the bottleneck, and the card asks
                                // one question of it.
                                player["research_boosted"] = json!(seat
                                    .research
                                    .as_ref()
                                    .is_some_and(|tech| seat.boosted_techs.contains(&Name::new(tech))));
                                player["civic_boosted"] = json!(seat
                                    .civic
                                    .as_ref()
                                    .is_some_and(|civic| seat.boosted_civics.contains(&Name::new(civic))));
                            }
                        }
                    }
                }
            }
            // League identity: who is playing each seat and how strong the
            // league currently believes they are on this civ.
            self.name_seat_ratings(&mut o);
            self.name_ai_players(&mut o);
            o["spectate"] = json!(true);
            o["supervised"] = json!(self.params.supervised);
            o["spectator_paused"] = json!(self.spectator_paused);
            o["view_player"] = json!(self.view_player);
            o["leader_pool"] = json!(self.game.leader_pool.id());
            o["turn_structure"] = json!(self.game.turn_structure.id());
            if self.game.turn_structure == TurnStructure::Simultaneous {
                // The regime's health instrument, for the same panel that
                // names the regime: how many plans the world has outrun.
                o["simultaneous"] = json!(self.simultaneous_census);
            }
            o["teams"] = json!(major_teams(&self.game));
            o["victory_conditions"] = json!(self.game.victory_conditions);
            o["legal_actions"] = json!([]);
            // Lets a long-running spectator notice that its server was
            // rebuilt/restarted between games and reload the latest UI.
            o["server_instance"] = json!(process_identity());
            o["server_commit"] = json!(runtime_commit("unknown"));
            o["server_commit_time"] = json!(runtime_commit_time());
            o["server_built_at"] = json!(runtime_built_at());
            o["server_artifact_bytes"] = json!(runtime_artifact_bytes());
            o["server_artifact_kind"] = json!(runtime_artifact_kind());
            return o;
        }
        let mut o = observation(&self.game, 0);
        // A rival you have met has a standing, the same way a chess opponent
        // does, and the HUD has always had a column for it. It used to be
        // empty in every interactive game because only the spectator wrote
        // one. Their plan is still theirs; only the rating is public.
        self.name_seat_ratings(&mut o);
        self.name_ai_players(&mut o);
        self.name_human_players(&mut o);
        o["spectate"] = json!(false);
        o["supervised"] = json!(self.params.supervised);
        o["view_player"] = json!(0);
        o["leader_pool"] = json!(self.game.leader_pool.id());
        o["turn_structure"] = json!(self.game.turn_structure.id());
        o["teams"] = json!(major_teams(&self.game));
        o["victory_conditions"] = json!(self.game.victory_conditions);
        o["legal_actions"] = serde_json::to_value(self.game.legal_actions(0)).unwrap();
        o["server_instance"] = json!(process_identity());
        o["server_commit"] = json!(runtime_commit("unknown"));
        o["server_commit_time"] = json!(runtime_commit_time());
        o["server_built_at"] = json!(runtime_built_at());
        o["server_artifact_bytes"] = json!(runtime_artifact_bytes());
        o["server_artifact_kind"] = json!(runtime_artifact_kind());
        o
    }

    /// Spectator mode: play out exactly one player's turn with its AI.
    /// Returns the pid and successful actions so the observer UI can explain
    /// the AI's decisions instead of showing only their eventual outcomes.
    pub fn step(&mut self) -> (usize, Vec<Action>) {
        let log_start = self.game.log.len();
        let pid = self.step_quietly();
        let actions = self
            .game
            .log
            .since(log_start)
            .map(|(_, action)| action.clone())
            .collect();
        (pid, actions)
    }

    /// The same turn advance without materializing the action trace. The
    /// unattended pacer advances a whole exhibition this way — cloning every
    /// action of every turn for a reader that does not exist was the only
    /// difference.
    pub fn step_quietly(&mut self) -> usize {
        let g = &mut self.game;
        let pid = g.current;
        if g.winner.is_some() {
            return pid;
        }
        // A simultaneous game advances one whole game turn per step: every
        // seat plans against the same frozen world and the plans commit
        // through the ordinary rules. Serial on purpose (`jobs = 1`) — the
        // browser build has no thread pool, and a watched game spends its
        // pace budget per turn rather than per seat. An aborted cycle is
        // never retried: the driver has already said a retry would replay
        // the same turn forever, so the world sits still instead.
        if g.turn_structure == TurnStructure::Simultaneous {
            if !self.simultaneous_census.aborted
                && !crate::simultaneous::step_cycle(g, &mut self.ais, 1, &mut self.simultaneous_census)
            {
                eprintln!(
                    "[server] simultaneous cycle aborted on turn {}: {}",
                    g.turn,
                    self.simultaneous_census.summary()
                );
            }
            self.record_league_result();
            return pid;
        }
        self.ais[pid].take_turn(g, pid);
        if g.current == pid && g.winner.is_none() {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        // Every way of advancing the world funnels through here — the browser
        // stepping a batch, the headless pacer running an unattended
        // exhibition, autoplay — so this is the one place a result cannot be
        // missed.
        self.record_league_result();
        pid
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
        // The ratings are a sequential-regime instrument. A simultaneous
        // game changes every seat's information set, so its results must
        // never blend into the same Glicko-2 table — the game plays and is
        // shown, but it rates nobody.
        if self.game.turn_structure == TurnStructure::Simultaneous {
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
        // Same ordering and competition ranks as a distributed league game:
        // the declared winner stands alone, while equal non-winner scores are
        // draws rather than arbitrary wins for the lower player id.
        let rated: Vec<usize> = (0..seat_names.len())
            .filter(|pid| seat_names[*pid].is_some())
            .collect();
        let (rated, ranks) = crate::league::competition_ranking(
            rated,
            Some(winner),
            |pid| self.game.score(pid),
        );
        let seats: Vec<crate::league::LiveGameSeat> = rated
            .iter()
            .enumerate()
            .map(|(place, pid)| {
                let civilization = self.game.players[*pid].civ.clone();
                let leader = self
                    .game
                    .rules
                    .civs
                    .get(&civilization)
                    .map(|spec| spec.leader.clone())
                    .unwrap_or_else(|| civilization.clone());
                crate::league::LiveGameSeat {
                    strategy: seat_names[*pid].clone().unwrap(),
                    leader,
                    civilization,
                    rank: ranks[place],
                    won: *pid == winner,
                }
            })
            .collect();
        let victory = self.game.victory_type.clone().unwrap_or_default();
        let Some(updated) = crate::league::record_ranked_game(
            &dir,
            &seats,
            self.game.seed,
            self.game.reported_turn(),
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
    /// A name is matched against the live-eligible league roster first — by
    /// entrant name or by the handle the leaderboards show — and then against
    /// the built-in agents, so a build with no roster on disk still has
    /// something to hand the seat to. An unknown or league-only name is an
    /// error rather than a silent fallback: a player who picked a strategy and
    /// got a different one has been lied to.
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
                    .find(|s| !s.league_only && (s.name == name || s.username == name))
                    .map(|s| s.kind.clone())
            })
            .or_else(|| {
                BUILTIN_STRATEGIES.iter().any(|(id, _)| *id == name).then(|| {
                    crate::league::StrategyKind::Builtin { ai: name.to_string() }
                })
            })
            .ok_or_else(|| format!("no strategy named {name}"))?;
        self.ais[seat] = crate::league::make_send_ai(&kind, seed);
        // A newly seated agent joins the same record as the rest of the table;
        // without this the seat a player just handed over goes quiet.
        self.ais[seat].attach_journal(self.journal.handle());
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
    EMBEDDED_INDEX_HTML.as_bytes().to_vec()
}

fn app_js() -> Vec<u8> {
    for p in ["web/assets/app.js"] {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    EMBEDDED_APP_JS.as_bytes().to_vec()
}

fn feature_atlas() -> Vec<u8> {
    std::fs::read("web/assets/feature-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_FEATURE_ATLAS.to_vec())
}

fn environment_feature_atlas() -> Vec<u8> {
    std::fs::read("web/assets/environment-feature-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_ENVIRONMENT_FEATURE_ATLAS.to_vec())
}

fn hidden_map_monsters() -> Vec<u8> {
    std::fs::read("web/assets/hidden-map-monsters.png")
        .unwrap_or_else(|_| EMBEDDED_HIDDEN_MAP_MONSTERS.to_vec())
}

fn civ6_unit_flags() -> Vec<u8> {
    std::fs::read("web/assets/civ6-unit-flags.png")
        .unwrap_or_else(|_| EMBEDDED_CIV6_UNIT_FLAGS.to_vec())
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

fn viewer_path(path: &str) -> bool {
    matches!(path, "/" | "/index.html" | "/rust" | "/rust/")
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
/// `world:turn:finished:sequence`. All four parts matter: a victory may be
/// decided during a seat's step without incrementing the turn, and at Blitz
/// and slower several completed player turns share one world turn. Legacy
/// two- and three-part tokens name sequence zero, which remains the opening
/// frame of a process. Anything unparseable simply means no baseline, and the
/// page is sent the whole map.
fn held_frame(token: &str) -> Option<SpectatorFrame> {
    let mut fields = token.split(':');
    let seed = fields.next()?.parse().ok()?;
    let turn = fields.next()?.parse().ok()?;
    let finished = match fields.next() {
        None | Some("0") => false,
        Some("1") => true,
        Some(_) => return None,
    };
    let sequence = match fields.next() {
        None => 0,
        Some(sequence) => sequence.parse().ok()?,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(SpectatorFrame {
        seed,
        turn,
        finished,
        sequence,
    })
}

/// The pre-game teams of a world, one entry per major seat in seat order;
/// `null` is a seat playing for itself.
///
/// The lobby reads its own Teams control back out of this, so a page that
/// reloads over a team game offers to restart *that* game rather than a
/// free-for-all with the same map.
fn major_teams(game: &Game) -> Vec<Option<usize>> {
    game.players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.team)
        .collect()
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
        "seed": params.seed,
        "players": params.num_players,
        "width": params.width,
        "height": params.height,
        "city_states": params.num_city_states,
        "turns": params.max_turns,
        "base_ruleset": params.base_ruleset.id(),
        "start_era": start_era_id(params.start_era),
        "future_era": future_era_id(params.future_era),
        "turn_structure": turn_structure_id(params.turn_structure),
        "map": params.map_script.id(),
        "shape": params.map_topology.id(),
        "poles": params.map_poles.id(),
        "speed": params.game_speed.id(),
        "leader_pool": params.leader_pool.id(),
        "teams": params.teams,
        "victories": victories,
    })
}

/// The agents a person can hand their seat to, strongest first.
///
/// With a league roster on disk this is every live-eligible entrant still
/// competing, with the rating it is defending, so the choice is between *our*
/// strategies and not between adjectives. Offline-only entrants stay on the
/// rating schedule without becoming an offer this server cannot yet afford.
/// An entrant that has not played a rated game yet is marked provisional
/// rather than shown as an authoritative 1500. Without a roster the list falls
/// back to the built-in agents, because a control with nothing in it is worse
/// than one with four honest entries.
fn strategy_roster(session: &Session) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    if let Some(roster) = session.roster.as_ref() {
        // Agents only. A person registered in this roster is a player in it,
        // but a seat cannot be handed to somebody who is not at a keyboard.
        let mut active: Vec<&crate::league::Strategy> = roster
            .strategies
            .iter()
            .filter(|s| !s.retired && !s.human && !s.league_only)
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

/// The ELO choices the custom setup table may name. Ratings are sparse by
/// design: a combination only appears after an entrant has actually played
/// that leader and civilization. Keep the strategy identity beside each
/// number so a later custom-seat protocol can bind the choice to the exact
/// entrant instead of treating equal-looking ratings as interchangeable.
fn leader_elo_options(session: &Session) -> Value {
    let mut combinations: BTreeMap<(String, String), BTreeMap<i64, BTreeSet<String>>> =
        BTreeMap::new();
    let Some(roster) = session.roster.as_ref() else {
        return json!([]);
    };
    for strategy in roster
        .strategies
        .iter()
        .filter(|strategy| !strategy.retired && !strategy.human && !strategy.league_only)
    {
        let strategy_name = if strategy.username.is_empty() {
            strategy.name.clone()
        } else {
            strategy.username.clone()
        };
        for (leader, civs) in &strategy.leader_elo {
            for (civ, rating) in civs {
                if rating.games == 0 || !rating.rating.is_finite() {
                    continue;
                }
                combinations
                    .entry((civ.clone(), leader.clone()))
                    .or_default()
                    .entry(rating.rating.round() as i64)
                    .or_default()
                    .insert(strategy_name.clone());
            }
        }
    }
    json!(combinations
        .into_iter()
        .map(|((civ, leader), elos)| json!({
            "civ": civ,
            "leader": leader,
            "elos": elos
                .into_iter()
                .rev()
                .map(|(elo, strategies)| json!({
                    "elo": elo,
                    "strategies": strategies.into_iter().collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
        }))
        .collect::<Vec<_>>())
}

/// An explicit turn cap is a real configuration value, not a cast through an
/// arbitrary JSON number. Zero cannot produce a playable game and a number
/// wider than the engine's `u32` cap used to wrap into a surprising length.
fn requested_turn_limit(request: &Value) -> Option<u32> {
    request["max_turns"]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
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
    if let Some(v) = request["base_ruleset"].as_str().and_then(BaseRuleset::from_id) {
        p.base_ruleset = v;
    }
    // A rung nobody has built yet is refused rather than substituted: a lobby
    // that asks for the Stone Age and is quietly handed the Ancient era has
    // been lied to. `start_era_from_id` answers with nothing for an unbuilt
    // rung exactly as it does for an unknown one, so the previous setting
    // stands and the client can see it did.
    if let Some(era) = request["start_era"].as_str().and_then(start_era_from_id) {
        p.start_era = era;
    }
    // Same contract for the other end of the game: an era nobody has built is
    // refused rather than quietly played as the classic one.
    if let Some(era) = request["future_era"].as_str().and_then(future_era_from_id) {
        p.future_era = era;
    }
    // The turn structure follows the same refuse-unknown contract. Whether a
    // simultaneous request can be *honoured* also depends on who is seated,
    // which the end of this function settles once every field has landed.
    if let Some(v) = request["turn_structure"]
        .as_str()
        .and_then(crate::setup::turn_structure_from_id)
    {
        p.turn_structure = v;
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
    // Heat was a boolean once — poles on or off. Only its `true` still names a
    // world that exists, and that world is the default anyway, so a client
    // sending the old boolean is left where it already was rather than being
    // pushed into the one remaining alternative it never asked for.
    if request["map_poles"].as_bool() == Some(true) {
        p.map_poles = MapPoles::Poles;
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
    if let Some(v) = requested_turn_limit(request) {
        p.max_turns = v;
    }
    if let Some(v) = request["leader_pool"].as_str().and_then(LeaderPool::from_id) {
        p.leader_pool = v.available_or_default();
    }
    let selected_pool = p.leader_pool;
    p.civs.retain(|civ| {
        leader_roster::entry(civ).is_some_and(|entry| entry.available && entry.pool == selected_pool)
    });
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
        let selected_pool = p.leader_pool;
        p.civs = civs
            .iter()
            .filter_map(|civ| civ.as_str())
            .filter(|civ| {
                leader_roster::entry(civ)
                    .is_some_and(|entry| entry.available && entry.pool == selected_pool)
            })
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
        // Each city-state needs a distinct identity and Suzerain bonus. The
        // same cap is exposed through `/rules` for the browser and applies to
        // direct clients as a final guard.
        p.num_city_states = usize::try_from(v)
            .unwrap_or(usize::MAX)
            .min(Rules::shared().city_states.roster.len());
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
            p.max_turns = requested_turn_limit(request).unwrap_or(spec.turns);
        }
    }
    // Only a spectated table plays the simultaneous regime. A human seat is
    // consulted live, one seat at a time — sequential by construction — so
    // the structure is refused for a played game exactly as `play` refuses
    // it at launch, and the rest of the request still lands.
    if !p.spectate && p.turn_structure == TurnStructure::Simultaneous {
        p.turn_structure = TurnStructure::Sequential;
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
        // A game somebody is playing is not stepped from here, and this loop
        // must not hold the frame gate while it idles past it.
        //
        // The gate orders this loop against a viewer's *first* snapshot, so
        // the opening `/state?painted=` of every page blocks on it. Taking it,
        // sleeping 300ms and immediately retaking it leaves a window too narrow
        // to win against a thread that is already parked on the mutex, and a
        // single-player page then never finishes booting: its opening read is
        // starved until the page gives up at fifteen seconds, retries, and is
        // starved again. So decide before taking the gate, not after.
        //
        // Read it into a bool first: a guard built in an `if` condition lives
        // until the end of the whole `if`, so testing the lock inline would
        // hold the *session* across the sleep below and stall every request
        // that needs it.
        let being_played_by_hand = !sh.session.lock().unwrap().params.spectate;
        if being_played_by_hand {
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }
        // Close the first-viewer race as well as the steady-state one. A page
        // attaching to the current turn must either finish registering and
        // receive that snapshot before this step begins, or wait until the
        // step completes and receive the next snapshot as its first frame.
        // Once registered, its current frame must be painted before any more
        // simulation work starts.
        //
        // Wait for that frame *before* taking the gate, then take it and
        // confirm nobody seated itself in between. Waiting while holding the
        // gate deadlocked the two halves against each other: the only way to
        // register as a viewer is an opening `/state?painted=`, which needs
        // this same gate, so a page arriving while the loop waited for some
        // *other* viewer could not get in — and a restart, whose whole job is
        // to put a new page in front of a new world, is exactly that arrival.
        // Measured on a recorded 74x46 six-player profile: 0 turns in 25
        // seconds.
        // A re-check under the gate is as atomic as waiting inside it was,
        // because seating happens under the gate too.
        let simulation_frame_gate = loop {
            let current_frame = {
                let s = sh.session.lock().unwrap();
                spectator_frame(&s.game, sh.frame_sequence.load(Ordering::Relaxed))
            };
            sh.wait_for_turn_frame(current_frame);
            let gate = sh.simulation_frame_gate.lock().unwrap();
            if !sh.frame_outstanding(current_frame) {
                break gate;
            }
        };
        // A request can pause while this loop is waiting for the frame gate.
        // Check again after entering it so a play-on-and-pause transition can
        // clear the winner without one already-admitted AI step slipping past
        // the new pause.
        if sh.paused.load(Ordering::Relaxed) {
            over_since = None;
            watched_turn = None;
            drop(simulation_frame_gate);
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
                // Seating can change between the check above and this one, so
                // this stays as the authority — but it releases the gate
                // before idling, for the same reason.
                drop(s);
                drop(simulation_frame_gate);
                std::thread::sleep(Duration::from_millis(300));
                continue;
            }
            if s.game.winner.is_some() {
                let t0 = *over_since.get_or_insert_with(Instant::now);
                let left = final_countdown_ms(&sh)
                    .saturating_sub(t0.elapsed().as_millis() as u64);
                sh.restart_in.store(left, Ordering::Relaxed);
                if left == 0 {
                    s.start_automatic_next_game(sh.take_next_game_params());
                    sh.current_seed.store(s.game.seed, Ordering::Relaxed);
                    sh.adopt_live_params(&s.params);
                    over_since = None;
                    watched_turn = None;
                    sh.restart_in.store(u64::MAX, Ordering::Relaxed);
                    // A world's opening turn is a turn, and it is the one turn
                    // no seat has to complete for it to exist. Gate it like
                    // any other or the stepper plays straight through the
                    // starting position — settlers before their capitals —
                    // and the first thing a viewer ever sees of a new world is
                    // already several turns into it.
                    let sequence = sh.frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    completed_frame = Some(spectator_frame(&s.game, sequence));
                }
                delay = 200;
                waiting = true;
            } else {
                over_since = None;
                sh.restart_in.store(u64::MAX, Ordering::Relaxed);
                let step_started = Instant::now();
                let turn_before = s.game.turn;
                let finished_before = s.game.winner.is_some();
                let pid = s.step_quietly();
                turn_compute_us += step_started.elapsed().as_micros() as u64;
                if spectator_step_completes_frame(pace, turn_before, finished_before, &s.game) {
                    let sequence = sh.frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    completed_frame = Some(spectator_frame(&s.game, sequence));
                }
                // The step that ends a game has to hand the viewer its
                // countdown in the same breath. Arming it on the next pass
                // instead left `/state` reporting no countdown at all for a
                // beat, so the result screen opened on "preparing the next
                // world" and only then began counting. The viewer must get
                // every millisecond of the selected window to choose one more
                // turn.
                if s.game.winner.is_some() {
                    over_since = Some(Instant::now());
                    sh.restart_in
                        .store(final_countdown_ms(&sh), Ordering::Relaxed);
                }
                // A turn is one step per seat, so a seat waits for its own
                // share of the turn budget and the round adds up to the pace.
                // Only the living take a step: counting the eliminated made a
                // late game outrun its own pace as the city-states fell.
                // A simultaneous step *is* the whole round, so it spends the
                // whole budget in one wait instead of a seat's share of it.
                if s.game.turn_structure == TurnStructure::Simultaneous {
                    delay = pace;
                } else {
                    let living: Vec<_> = s.game.players.iter().filter(|p| p.alive).collect();
                    let minors = living
                        .iter()
                        .filter(|p| p.is_minor || p.is_barbarian)
                        .count();
                    let majors = living.len() - minors;
                    let p = &s.game.players[pid];
                    delay = seat_delay_ms(pace, majors, minors, p.is_minor || p.is_barbarian);
                }
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
        // before simulation continues. Lightning publishes at the end of the
        // round; Blitz and slower publish here after every player's turn, so
        // majors move one frame at a time before city-states and barbarians do.
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
            // Unlimited: no wait between steps. Yield anyway, and after each
            // work slice rest long enough to hold the stepper near 90% of a
            // core and give the single-threaded accept loop a real slot, or
            // /state would starve behind the session lock.
            if unlimited_since.elapsed() >= Duration::from_millis(UNLIMITED_WORK_SLICE_MS) {
                std::thread::sleep(Duration::from_millis(UNLIMITED_REST_MS));
                unlimited_since = Instant::now();
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
/// The world the setup panel is describing, normalized against the live one.
///
/// A free function because the queue has two homes: `Shared` on a server, and
/// a thread local in the wasm build, which has no threads to share between.
fn staged_next_game_params(base: &Params, request: &Value) -> Params {
    let mut params = new_game_params(base, request);
    params.spectate = base.spectate;
    params
}

fn decorate(o: &mut Value, sh: &Shared) {
    let r = sh.restart_in.load(Ordering::Relaxed);
    if r != u64::MAX {
        // `restart_in` remains the compact, backwards-compatible display
        // value. The browser also needs the unrounded remainder so it can
        // paint the seconds between these expensive full-state responses
        // instead of using their one-second long-poll cadence as a clock.
        o["restart_in_ms"] = json!(r);
        o["restart_in"] = json!(r.div_ceil(1000));
    }
    o["pace"] = json!(sh.pace_ms.load(Ordering::Relaxed));
    o["between_game_countdown_ms"] =
        json!(sh.between_game_countdown_ms.load(Ordering::Relaxed));
    o["paused"] = json!(sh.paused.load(Ordering::Relaxed));
    o["frame_sequence"] = json!(sh.frame_sequence.load(Ordering::Relaxed));
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
    // The request lives beside the session rather than inside it, so it is
    // attached here for every reader that used to find it in `state()`.
    if let Some(request) = sh.pending_new_game_request() {
        o["supervisor_request"] = request;
        // A world being handed over is not a world that has stalled: the
        // supervisor reads this before deciding to nudge it.
        o["spectator_paused"] = json!(true);
    } else {
        o["supervisor_request"] = Value::Null;
    }
    o["next_game_settings"] = sh.staged_next_game_settings();
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
        ("GET", path) if viewer_path(path) => {
            respond(stream, "200 OK", "text/html; charset=utf-8", &index_html());
        }
        ("GET", "/assets/app.js") => {
            respond(
                stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                &app_js(),
            );
        }
        ("GET", "/assets/feature-atlas.png") => {
            respond(stream, "200 OK", "image/png", &feature_atlas());
        }
        ("GET", "/assets/environment-feature-atlas.png") => {
            respond(stream, "200 OK", "image/png", &environment_feature_atlas());
        }
        ("GET", "/assets/hidden-map-monsters.png") => {
            respond(stream, "200 OK", "image/png", &hidden_map_monsters());
        }
        ("GET", "/assets/civ6-unit-flags.png") => {
            respond(stream, "200 OK", "image/png", &civ6_unit_flags());
        }
        // A lock-free identity probe for supervised process handoffs. The
        // browser used to fetch the multi-megabyte `/state` document here and
        // could queue behind an AI step for its entire three-second timeout.
        ("GET", "/runtime") => {
            respond_json(
                stream,
                &json!({
                    "server_instance": process_identity(),
                    "seed": sh.current_seed.load(Ordering::Relaxed),
                    "commit": runtime_commit("unknown"),
                    "commit_time": runtime_commit_time(),
                    "built_at": runtime_built_at(),
                    "artifact_bytes": runtime_artifact_bytes(),
                    "artifact_kind": runtime_artifact_kind(),
                    // The setup queue is beside the simulation lock too. The
                    // supervisor needs the viewer's latest choice even when
                    // an AI turn is making the full `/state` response wait.
                    "next_game_settings": sh.staged_next_game_settings(),
                    // The supervisor's only view of a restart used to be
                    // `/state`, which is exactly what a long AI turn makes
                    // unavailable. This probe takes no lock the simulation
                    // holds, so a restart is seen while the turn runs.
                    "supervisor_request": sh.pending_new_game_request(),
                }),
            );
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
                    query_value(&request_target, "finished"),
                    query_value(&request_target, "frame").map(str::parse::<u64>),
                ) {
                    (Ok(turn), Some(Ok(seed)), Some("0") | None, Some(Ok(sequence))) => Some(SpectatorFrame {
                        seed,
                        turn,
                        finished: false,
                        sequence,
                    }),
                    (Ok(turn), Some(Ok(seed)), Some("1"), Some(Ok(sequence))) => Some(SpectatorFrame {
                        seed,
                        turn,
                        finished: true,
                        sequence,
                    }),
                    _ => None, // a page that has painted nothing yet
                };
                sh.note_viewer_request(&viewer, painted);
            }
            // A page that says what it holds is asking for the next turn, not
            // this one again, so it waits here instead of spinning on a clock
            // of its own. A reader that names nothing is answered at once.
            sh.wait_for_next_turn(have);
            // A page watching the AI reason says which thought it last holds,
            // and gets what has been recorded since. Absent, nothing is sent:
            // the keeper's refresh probe and any curl read `/state` too, and
            // none of them are reading the reasoning log.
            let wants_reasoning = query_value(&request_target, "think")
                .map(|cursor| cursor.parse::<u64>().unwrap_or(0));
            let (mut o, frame) = {
                let session = sh.session.lock().unwrap();
                let frame = spectator_frame(
                    &session.game,
                    sh.frame_sequence.load(Ordering::Relaxed),
                );
                let mut observed = session.state();
                observed["frame_sequence"] = json!(frame.sequence);
                if wants_planet {
                    if let Some(geometry) = crate::obs::planet_geometry(&session.game) {
                        observed["map"]["planet"] = geometry;
                    }
                }
                if let Some(cursor) = wants_reasoning {
                    observed["ai_reasoning"] = session.reasoning_json(cursor);
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
        // The panel polls this independently of the game state: sampling the
        // host must never make a viewer serialize its whole map just to decide
        // whether to prefer a lighter display treatment.
        ("GET", "/machine-metrics") => {
            respond_json(stream, &machine_metrics_json());
        }
        // Everything a supervisor needs to know - is there a game, is it over -
        // without building the whole observation. /state runs close to a
        // megabyte of JSON on a standard map, and something polling it every
        // few seconds to read one field spends the server's time on rendering
        // a view nobody looks at.
        ("GET", "/status") => {
            let (frames_missed, frames_painted, viewers, autoplay_turns) = sh.frame_audit();
            let session = sh.session.lock().unwrap();
            let game = &session.game;
            respond_json(
                stream,
                &json!({
                    "turn": game.turn,
                    "winner": game.winner,
                    "victory_type": game.victory_type,
                    "spectate": session.params.spectate,
                    // Published frames no viewer ever drew. At Blitz and
                    // slower this counts player turns, not only round turns.
                    "frames_missed": frames_missed,
                    // The last turn a viewer reported drawing; null when
                    // nobody is watching, which is why zero misses on its own
                    // is not yet good news.
                    "frames_painted": frames_painted.map(|frame| frame.turn),
                    "frames_painted_sequence": frames_painted.map(|frame| frame.sequence),
                    "frame_sequence": sh.frame_sequence.load(Ordering::Relaxed),
                    // How many pages that promise is being kept to. Each is
                    // waited for separately, so this is also the number of
                    // paints a turn now costs before the next one starts.
                    "viewers": viewers,
                    // Turns `POST /autoplay` has simulated. A human game is
                    // not stepped by the exhibition loop and its page sends no
                    // painted acknowledgements, so auto-play contributes
                    // nothing to `frames_painted` and used to contribute
                    // nothing to `frames_missed` either — it could drop nine
                    // turns in ten and still read clean. This is the count
                    // those misses are out of: zero here means auto-play was
                    // never pressed, not that it behaved.
                    "autoplay_turns": autoplay_turns,
                    // Which code is actually playing. A binary swap only
                    // happens between games, so a running server is always
                    // somewhat behind origin/main and there was no way to see
                    // by how much - "is it running old code" could only be
                    // guessed at from file timestamps. The supervisor passes
                    // it when it launches the promoted binary; an unstamped
                    // build reports unknown.
                    "commit": runtime_commit("unknown"),
                    "commit_time": runtime_commit_time(),
                    "built_at": runtime_built_at(),
                }),
            );
        }
        ("POST", "/pace") => {
            if let Some(v) = parsed["ms"].as_u64() {
                // 0 is the unlimited pace; anything else is a turn budget.
                sh.pace_ms.store(v.min(60_000), Ordering::Relaxed);
                sh.turn_us.store(0, Ordering::Relaxed); // re-measure at the new pace
            }
            let countdown_error = parsed["between_game_countdown_ms"]
                .as_u64()
                .and_then(|value| match valid_between_game_countdown_ms(value) {
                    Some(value) => {
                        sh.between_game_countdown_ms
                            .store(value, Ordering::Relaxed);
                        None
                    }
                    None => Some(
                        "between-game countdown must be one of 0, 3000, 5000, or 10000 milliseconds",
                    ),
                });
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
            if let Some(error) = countdown_error {
                o["error"] = json!(error);
            }
            respond_json(stream, &o);
        }
        ("GET", "/save") => {
            let session = sh.session.lock().unwrap();
            let save = serde_json::to_value(&session.game).unwrap();
            respond_json(stream, &save);
        }
        // The district adjacency calculator over the live game: every
        // buildable district for a city's civilization, every legal plot,
        // what each would earn there and the ledger saying why — foundations
        // counted as the districts they will become. A planning read for
        // spectator tools and the Civ 6 bridge's site advisor; read-only.
        // `?city=<id>` narrows to one city, the default is every city.
        ("GET", "/adjacency") => {
            let session = sh.session.lock().unwrap();
            let only: Option<u32> = query_value(&request_target, "city")
                .and_then(|city| city.parse().ok());
            let cities: Vec<Value> = session
                .game
                .cities
                .iter()
                .filter(|(cid, _)| only.is_none_or(|wanted| wanted == **cid))
                .map(|(cid, city)| {
                    json!({
                        "id": cid,
                        "name": city.name,
                        "owner": city.owner,
                        "forecasts": session.game.district_adjacency_calculator(*cid),
                    })
                })
                .collect();
            drop(session);
            respond_json(stream, &json!({"cities": cities}));
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
                    sh.current_seed
                        .store(session.game.seed, Ordering::Relaxed);
                    sh.adopt_live_params(&session.params);
                    if let Some(queued) = session.take_resumed_next_game_params() {
                        *sh.next_game_params.lock().unwrap() = Some(queued);
                    }
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
                    "city_state_limit": r.city_states.roster.len(),
                    "civ6_leaders": CIV6_LEADER_POOL.as_slice(),
                    "leader_pools": leader_roster::browser_pools(),
                    "great_people": r.great_people, "governors": r.governors,
                    "map_sizes": CIV6_MAP_SIZES,
                    "difficulties": r.difficulties, "speeds": r.speeds,
                    "base_rulesets": BASE_RULESETS,
                    "start_eras": START_ERAS,
                    "future_eras": FUTURE_ERAS,
                    "map_scripts": CIV6_MAP_SCRIPTS,
                    "map_topologies": MAP_TOPOLOGIES,
                    "map_poles": MAP_POLES,
                    "game_speeds": CIV6_GAME_SPEEDS,
                    // The other game's vocabulary, for the mode that plays it.
                    // It never changes while a server runs, unlike the host
                    // report at `/civ6`, which is a different question asked of
                    // a different machine's installation.
                    "civ6": civ6::vocabulary(),
                    "strategies": strategy_roster(&session),
                    "leader_elo_options": leader_elo_options(&session),
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
                    .is_some_and(|instance| instance != process_identity() as u64)
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
            // One response carries one state, so every turn in this batch past
            // the first was played where nobody could see it. Recorded after
            // the session lock is released, and only on the path that actually
            // simulated: the retry arm above answers from
            // `last_autoplay_request` without playing anything, and counting
            // there would charge a dropped response twice.
            sh.frame_delivery
                .lock()
                .unwrap()
                .turns_simulated_without_a_frame(played);
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
            let mut completed_frame = None;
            if session.params.spectate {
                let count = parsed["count"].as_u64().unwrap_or(1) as usize;
                let steps = session.step_many(count);
                if !steps.is_empty() {
                    let sequence = sh.frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    completed_frame = Some(spectator_frame(&session.game, sequence));
                }
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
            if let Some(frame) = completed_frame {
                sh.note_turn_ready(frame);
            }
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        // Carry the decided world on instead of retiring it. `paused` is part
        // of the same transition: the frame gate makes "look around" clear the
        // winner and stop the stepper atomically, rather than racing a second
        // request to /pace against the first continued turn.
        ("POST", "/play-on") => {
            let mode_name = parsed["mode"].as_str().unwrap_or("until_next_victory");
            let Some(mode) = PlayOnMode::parse(mode_name) else {
                respond_json(
                    stream,
                    &json!({"error": format!("unknown play-on mode {mode_name:?}")}),
                );
                return;
            };
            let requested_pause = parsed.get("paused").and_then(Value::as_bool);
            let simulation_frame_gate = sh.simulation_frame_gate.lock().unwrap();
            let mut session = sh.session.lock().unwrap();
            let played_on = session.play_on(mode);
            if played_on {
                if let Some(paused) = requested_pause {
                    sh.paused.store(paused, Ordering::Relaxed);
                    session.spectator_paused = paused;
                }
                // Clear this before taking the response snapshot, so it can
                // never promise both a paused continuation and a new world.
                sh.restart_in.store(u64::MAX, Ordering::Relaxed);
            }
            let mut out = session.state();
            out["error"] = if played_on {
                Value::Null
            } else {
                json!("this game has no result to play on past")
            };
            drop(session);
            drop(simulation_frame_gate);
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
            // No session lock, for the same reason `/supervisor-new` has none:
            // every `change` on the setup panel comes through here, and the
            // restart that follows waits for this write to land.
            sh.stage_next_game_settings(&parsed);
            respond_json(
                stream,
                &json!({"ok": true, "next_game_settings": sh.staged_next_game_settings()}),
            );
        }
        ("POST", "/new") => {
            let mut session = sh.session.lock().unwrap();
            let result = session.start_new_game(&parsed);
            if result.is_ok() {
                sh.current_seed
                    .store(session.game.seed, Ordering::Relaxed);
                sh.adopt_live_params(&session.params);
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
        // What this computer can do about the Civilization VI mode, and what a
        // run it can see is doing. Answered without the simulation lock: it is
        // a question about the machine and another game's files, and a page
        // watching a simulation must be able to ask it between turns.
        //
        // The verification-only mode is available on every computer and
        // refused on the ones that cannot run it, so this always answers. A
        // silent no is what made a dead Steam client cost eleven ladder
        // attempts.
        ("GET", "/civ6") => {
            let host = civ6::Host::probe();
            respond_json(
                stream,
                &json!({
                    "ready": host.ready(),
                    "install": host.install,
                    "controller": host.controller,
                    "blocked": host.blocked,
                    "holder": host.holder,
                    "run": host.run,
                }),
            );
        }
        // Start a real game of Civilization VI with the lobby's settings.
        //
        // The reply is a receipt, not a game: bringing the other game up takes
        // about three minutes on this install, and the run outlives both this
        // request and the page that made it. Progress is read back from
        // `GET /civ6`.
        ("POST", "/civ6/start") => {
            let started = civ6::Request::from_settings(&parsed).and_then(|request| {
                let tag = parsed["tag"]
                    .as_str()
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| civ6::new_tag(std::time::SystemTime::now()));
                civ6::start(&request, &tag)
            });
            respond_json(
                stream,
                &match started {
                    Ok(run) => json!({"error": Value::Null, "started": run}),
                    Err(error) => json!({"error": error, "started": Value::Null}),
                },
            );
        }
        ("POST", "/supervisor-new") => {
            // Answered without the simulation lock, and answered *small*. The
            // page reads only `error` from this, and building a full
            // observation for it would put the one control that escapes a
            // wedged turn right back behind that turn.
            let result = sh.request_supervised_new_game(&parsed);
            respond_json(
                stream,
                &json!({
                    "error": match result {
                        Ok(()) => Value::Null,
                        Err(error) => Value::String(error),
                    },
                    "server_instance": process_identity(),
                    "supervisor_request": sh.pending_new_game_request(),
                }),
            );
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
        automatic_successor_seed, chronicle_world_events, final_countdown_ms, held_frame,
        new_game_params, publishes_player_turn_frames, query_value, request_path, save_path,
        seat_delay_ms, spectator_frame, spectator_step_completes_frame, staged_next_game_params,
        strategy_roster, tile_mark, valid_between_game_countdown_ms, viewer_path, ChronicleSnapshot,
        ChronicleState, FrameDelivery, Params, Session, Shared, SpectatorFrame,
        BETWEEN_GAME_COUNTDOWN_OPTIONS_MS, DEFAULT_BETWEEN_GAME_COUNTDOWN_MS,
        EMBEDDED_CIV6_UNIT_FLAGS, EMBEDDED_HIDDEN_MAP_MONSTERS, EMBEDDED_INDEX,
        MAX_EXACT_JAVASCRIPT_INTEGER, SAVE_DIR, STATE_LONG_POLL, VIEWER_ACTIVE,
    };
    use crate::game::{
        Action, Game, LeaderPool, PlayOnMode, VictoryConditions, CIV6_LEADER_POOL,
    };
    use crate::server::{generated_ai_name, simulation_settings};
    use crate::setup::{
        future_era_from_id, start_era_from_id, BaseRuleset, FutureEra, GameSpeed, MapPoles,
        MapScript, MapTopology, TurnStructure, MAP_POLES,
    };
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{mpsc, Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn machine_metric_values_are_bounded_and_only_expose_percentages() {
        assert_eq!(super::bounded_percent(f64::NAN), None);
        assert_eq!(super::bounded_percent(-5.0), Some(0.0));
        assert_eq!(super::bounded_percent(140.0), Some(100.0));
        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                super::cpu_percent_from_ticks((100, 70), (200, 120)),
                Some(50.0)
            );
            assert_eq!(super::cpu_percent_from_ticks((100, 70), (100, 70)), None);
            assert_eq!(
                super::memory_percent_from_meminfo(
                    "MemTotal:       1000 kB\nMemAvailable:    250 kB\n",
                ),
                Some(75.0)
            );
        }
        #[cfg(target_os = "macos")]
        {
            let cpu = super::macos_top_cpu_percent(
                "CPU usage: 12.00% user, 3.00% sys, 85.00% idle\n",
            )
            .expect("CPU usage line should parse");
            assert!((cpu - 15.0).abs() < f64::EPSILON);
            assert_eq!(
                super::macos_memory_percent(
                    "Mach Virtual Memory Statistics: (page size of 4096 bytes)\n\
                     Pages free: 25.\nPages inactive: 25.\nPages speculative: 0.\n",
                    409_600,
                ),
                Some(50.0)
            );
        }
        assert_eq!(
            super::machine_metrics_value(super::MachineMetrics {
                cpu_percent: Some(12.31),
                memory_percent: None,
            }),
            json!({"cpu_percent": 12.3, "memory_percent": Value::Null})
        );
        assert_eq!(
            super::machine_metrics_value(super::MachineMetrics::default()),
            json!({"cpu_percent": Value::Null, "memory_percent": Value::Null})
        );
    }

    #[test]
    fn browser_offers_host_load_caps_and_display_guidance() {
        assert!(EMBEDDED_INDEX.contains("<span>Display Settings</span>"));
        for kind in ["cpu", "memory"] {
            assert!(EMBEDDED_INDEX.contains(&format!("id=\"{kind}-load-row\"")));
            assert!(EMBEDDED_INDEX.contains(&format!("id=\"{kind}-load-cap\"")));
        }
        for kind in ["cpu", "memory"] {
            let options = EMBEDDED_INDEX
                .split_once(&format!("id=\"{kind}-load-cap\""))
                .expect("load cap select")
                .1
                .split_once("</select>")
                .expect("load cap select closes")
                .0;
            assert_eq!(options.matches("<option").count(), 9);
            for percent in (10..=90).step_by(10) {
                assert!(
                    options.contains(&format!("<option value=\"{percent}\"")),
                    "{kind} cap should offer {percent}%"
                );
            }
        }
        for contract in [
            "DISPLAY_RESOURCE_CAPS_STORAGE_KEY",
            "fetchJSON(\"/machine-metrics\", {cache: \"no-store\"}, 3000)",
            "Prefer Performance resolution",
            "Machine telemetry is unavailable here.",
        ] {
            assert!(EMBEDDED_INDEX.contains(contract), "missing display load contract: {contract}");
        }
    }

    /// Lightning and Blitz resolve a step faster than an eye can find the
    /// token that took it, so a watcher can ask each unit's wake to linger a
    /// configured beat after the move lands. Pin the whole chain: the
    /// control, the persisted preference, the recorded wake, the lingering
    /// painter, and the ticker that keeps a fading wake animating.
    #[test]
    fn browser_lets_a_watcher_keep_unit_trails_lingering() {
        for contract in [
            "id=\"unittrails\" aria-label=\"Unit trail linger\"",
            "<option value=\"0\" selected>Move only</option>",
            "<option value=\"200\">Linger 0.2s</option>",
            "<option value=\"500\">Linger 0.5s</option>",
            "<option value=\"1000\">Linger 1s</option>",
            "<option value=\"2000\">Linger 2s</option>",
            "const UNIT_TRAIL_LINGER_STORAGE_KEY = \"civvis-unit-trail-linger\";",
            "const UNIT_TRAIL_LINGER_VALUES = [0, 200, 500, 1000, 2000];",
            "function setUnitTrailLinger(value) {",
            "trails.onchange = () => setUnitTrailLinger(trails.value);",
            "function drawLingeringUnitTrails(unitAlpha, now) {",
            "if (mapLens !== \"empire\") drawLingeringUnitTrails(unitAlpha, now);",
            "anim.trails.length > 0 ||",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(contract),
                "unit trail linger contract is missing: {contract}"
            );
        }
    }

    #[test]
    fn browser_gives_every_shipped_improvement_a_named_tile_marker() {
        let markers = EMBEDDED_INDEX
            .split_once("const IMPROVEMENT_MARKERS = Object.freeze({")
            .expect("the improvement marker registry")
            .1
            .split_once("\n});")
            .expect("the end of the improvement marker registry")
            .0;
        for improvement in crate::rules::Rules::embedded().improvements.keys() {
            assert!(
                markers.contains(&format!("\n  {}:", improvement.as_str())),
                "{improvement} has no deliberate browser tile marker"
            );
        }

        let renderer = EMBEDDED_INDEX
            .split_once("function drawImprovement(t, x, y) {")
            .expect("the improvement renderer")
            .1
            .split_once("\n// Every city drew")
            .expect("the end of the improvement renderer")
            .0;
        assert!(renderer.contains("const marker = IMPROVEMENT_MARKERS[imp] || \"unknown\";"));
        let marker_branch = renderer
            .find("} else if (marker !== \"unknown\") {")
            .expect("the named-marker branch");
        let anonymous_fallback = renderer
            .find("// Everything rarer still reads as a built installation")
            .expect("the anonymous fallback");
        assert!(
            marker_branch < anonymous_fallback,
            "known improvements must reach their marker before the generic fallback"
        );
        assert!(renderer.contains("paintImprovementMarker(marker, t, x, y, dir);"));
    }

    /// The pace a viewer picks is what a turn costs, so the seats' waits have
    /// to add back up to it — at any player count, with minors on their
    /// quarter beat. A per-seat pace made big games crawl at the same label.
    #[test]
    fn seat_waits_add_up_to_the_chosen_turn_pace() {
        for (majors, minors) in [(2, 0), (4, 4), (8, 12), (6, 3)] {
            for pace in [100, 500, 1_000, 4_000, 10_000] {
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
    fn player_turn_frames_begin_at_blitz() {
        assert!(!publishes_player_turn_frames(0), "Lightning stays round-by-round");
        assert!(!publishes_player_turn_frames(499));
        for pace in [500, 1_000, 2_000, 4_000, 60_000] {
            assert!(
                publishes_player_turn_frames(pace),
                "{pace}ms is Blitz or slower"
            );
        }
    }

    /// The simulation already seats Civ-style: majors first, then
    /// city-states, then its barbarian seat. At Blitz every one of those
    /// `Session::step` boundaries must become a distinct frame, while
    /// Lightning waits for the wrap. Capturing unit positions around the same
    /// boundary proves a movement is present in the acting player's frame,
    /// not deferred into the next civilization's paint.
    #[test]
    fn blitz_frames_follow_major_city_state_barbarian_turn_order() {
        let mut params = current();
        params.spectate = true;
        params.num_players = 3;
        params.num_city_states = 2;
        params.width = 30;
        params.height = 20;
        params.seed = 20_260_802;
        let mut session = Session::new(params);

        let expected: Vec<usize> = session
            .game
            .players
            .iter()
            .filter(|player| player.alive)
            .map(|player| player.id)
            .collect();
        let kinds: Vec<&str> = expected
            .iter()
            .map(|pid| {
                let player = &session.game.players[*pid];
                if !player.is_minor && !player.is_barbarian {
                    "major"
                } else if player.is_minor && !player.is_barbarian {
                    "city-state"
                } else {
                    "barbarian"
                }
            })
            .collect();
        assert_eq!(
            kinds,
            ["major", "major", "major", "city-state", "city-state", "barbarian"],
            "the live roster itself must follow Civilization's phase order"
        );

        let opening_turn = session.game.turn;
        let mut seen = Vec::new();
        let mut sequence = 0;
        let mut frames = Vec::new();
        let mut movement_seen = false;
        while session.game.turn == opening_turn && session.game.winner.is_none() {
            let acting = session.game.current;
            let positions = session
                .game
                .units
                .iter()
                .map(|(id, unit)| (*id, (unit.owner, unit.pos)))
                .collect::<std::collections::BTreeMap<_, _>>();
            let turn_before = session.game.turn;
            let finished_before = session.game.winner.is_some();
            let (stepped, _) = session.step();
            assert_eq!(stepped, acting);
            seen.push(stepped);

            assert!(spectator_step_completes_frame(
                500,
                turn_before,
                finished_before,
                &session.game,
            ));
            sequence += 1;
            frames.push(spectator_frame(&session.game, sequence));

            for (id, unit) in &session.game.units {
                if positions
                    .get(id)
                    .is_some_and(|(_, before)| *before != unit.pos)
                {
                    assert_eq!(
                        unit.owner, acting,
                        "seat {acting}'s frame included another player's unit movement"
                    );
                    movement_seen = true;
                }
            }

            let lightning_boundary = spectator_step_completes_frame(
                0,
                turn_before,
                finished_before,
                &session.game,
            );
            assert_eq!(
                lightning_boundary,
                session.game.turn != turn_before || session.game.winner.is_some(),
                "Lightning should publish only the round/result boundary"
            );
        }

        assert_eq!(seen, expected);
        assert!(
            frames.windows(2).all(|pair| pair[1].sequence == pair[0].sequence + 1),
            "each completed seat must receive its own frame identity"
        );
        assert!(movement_seen, "the opening round exercised no unit movement");
    }

    /// The setting is intentionally narrow: these are the exact choices on
    /// the page, and no arbitrary browser request can turn a result into an
    /// unbounded hold.
    #[test]
    fn between_game_countdown_is_limited_to_the_four_lobby_choices() {
        assert_eq!(BETWEEN_GAME_COUNTDOWN_OPTIONS_MS, [0, 3_000, 5_000, 10_000]);
        for value in BETWEEN_GAME_COUNTDOWN_OPTIONS_MS {
            assert_eq!(valid_between_game_countdown_ms(value), Some(value));
        }
        for value in [1, 2_999, 3_001, 7_000, 10_001] {
            assert_eq!(valid_between_game_countdown_ms(value), None);
        }

        let shared = shared_for(Session::new(current()));
        assert_eq!(final_countdown_ms(&shared), DEFAULT_BETWEEN_GAME_COUNTDOWN_MS);
        for value in BETWEEN_GAME_COUNTDOWN_OPTIONS_MS {
            shared
                .between_game_countdown_ms
                .store(value, Ordering::Relaxed);
            assert_eq!(final_countdown_ms(&shared), value);
        }

        assert!(EMBEDDED_INDEX.contains("id=\"between-game-countdown\""));
        assert!(EMBEDDED_INDEX.contains("<option value=\"0\">None</option>"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"3000\">3s</option>"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"5000\">5s</option>"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"10000\" selected>10s</option>"));
        assert!(EMBEDDED_INDEX.contains("const BETWEEN_GAME_COUNTDOWN_KEY"));
        assert!(EMBEDDED_INDEX.contains("betweenGameCountdownMs()"));
    }

    #[test]
    fn generated_ai_names_are_unique_and_follow_their_strategy() {
        let science: Vec<String> = (0..12)
            .map(|pid| generated_ai_name(42, pid, Some("science")))
            .collect();
        let unique: std::collections::BTreeSet<&str> =
            science.iter().map(|name| name.as_str()).collect();
        assert_eq!(unique.len(), science.len());
        assert!(science.iter().all(|name| ["Quantum", "Stellar", "Orbital", "Theory"]
            .iter()
            .any(|prefix| name.starts_with(prefix))));
        assert_ne!(
            generated_ai_name(42, 0, Some("science")),
            generated_ai_name(42, 0, Some("conquest"))
        );
        assert!(generated_ai_name(42, 0, None).starts_with("Resolute"));
    }

    #[test]
    fn a_mid_turn_victory_is_a_distinct_spectator_frame() {
        let playing = SpectatorFrame {
            seed: 41,
            turn: 7,
            finished: false,
            sequence: 0,
        };
        let won = SpectatorFrame {
            seed: 41,
            turn: 7,
            finished: true,
            sequence: 0,
        };
        assert_ne!(playing, won);
        assert_eq!(held_frame("41:7"), Some(playing));
        assert_eq!(held_frame("41:7:0"), Some(playing));
        assert_eq!(held_frame("41:7:1"), Some(won));
        assert_eq!(
            held_frame("41:7:0:23"),
            Some(SpectatorFrame {
                sequence: 23,
                ..playing
            })
        );
        assert_eq!(held_frame("41:7:maybe"), None);
    }

    #[test]
    fn a_mid_turn_victory_wakes_the_waiting_state_request_immediately() {
        let mut session = Session::new(current());
        session.game.turn = 7;
        session.game.winner = None;
        let seed = session.game.seed;
        let shared = Arc::new(Shared {
            current_seed: AtomicU64::new(seed),
            supervisor_request: Mutex::new(None),
            live_params: Mutex::new(session.params.clone()),
            next_game_params: Mutex::new(session.take_resumed_next_game_params()),
            session: Mutex::new(session),
            pace_ms: AtomicU64::new(0),
            between_game_countdown_ms: AtomicU64::new(DEFAULT_BETWEEN_GAME_COUNTDOWN_MS),
            paused: AtomicBool::new(false),
            restart_in: AtomicU64::new(u64::MAX),
            turn_us: AtomicU64::new(0),
            turn_compute_us: AtomicU64::new(0),
            frame_sequence: AtomicU64::new(0),
            frame_delivery: Mutex::new(FrameDelivery::default()),
            frame_painted: Condvar::new(),
            simulation_frame_gate: Mutex::new(()),
            latest: Mutex::new(None),
            turn_ready: Condvar::new(),
        });
        let playing = SpectatorFrame {
            seed,
            turn: 7,
            finished: false,
            sequence: 0,
        };
        let won = SpectatorFrame {
            finished: true,
            ..playing
        };
        let (sent, received) = mpsc::channel();
        let waiter = shared.clone();
        let started = Instant::now();
        std::thread::spawn(move || {
            waiter.wait_for_next_turn(Some(playing));
            sent.send(started.elapsed()).unwrap();
        });

        std::thread::sleep(Duration::from_millis(40));
        shared.session.lock().unwrap().game.winner = Some(0);
        shared.note_turn_ready(won);

        let elapsed = received
            .recv_timeout(Duration::from_millis(300))
            .expect("the terminal frame should wake the long poll");
        assert!(
            elapsed < Duration::from_millis(250),
            "terminal state waited for the {:?} long-poll cap: {elapsed:?}",
            STATE_LONG_POLL
        );
    }

    #[test]
    fn every_turn_waits_for_its_frame_only_while_a_viewer_is_active() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame {
            seed: 41,
            turn: 7,
            finished: false,
            sequence: 7,
        };
        let turn_8 = SpectatorFrame {
            seed: 41,
            turn: 8,
            finished: false,
            sequence: 8,
        };
        let next_world = SpectatorFrame {
            seed: 42,
            turn: 7,
            finished: false,
            sequence: 0,
        };
        let final_turn_7 = SpectatorFrame {
            finished: true,
            ..turn_7
        };
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
        assert!(delivery.wait_remaining(final_turn_7, now).is_some());

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
        let turn_7 = SpectatorFrame {
            seed: 41,
            turn: 7,
            finished: false,
            sequence: 7,
        };
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
        let turn_8 = SpectatorFrame {
            seed: 41,
            turn: 8,
            finished: false,
            sequence: 8,
        };
        delivery.viewer_request("one", None, later);
        delivery.frame_delivered("one", turn_8, later);
        delivery.viewer_request("one", Some(turn_8), later);
        assert_eq!(delivery.wait_remaining(turn_8, later), None);
        assert_eq!(delivery.seats.len(), 1, "the departed tab was retired");
    }

    #[test]
    fn only_the_exact_delivered_snapshot_can_acknowledge_a_complete_frame() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame {
            seed: 41,
            turn: 7,
            finished: false,
            sequence: 7,
        };
        let turn_8 = SpectatorFrame {
            seed: 41,
            turn: 8,
            finished: false,
            sequence: 8,
        };
        let other_world = SpectatorFrame {
            seed: 42,
            turn: 7,
            finished: false,
            sequence: 0,
        };
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

    /// A batch of `n` turns answers with one state, so `n - 1` of them were
    /// played where no page could draw them.
    ///
    /// This is arithmetic on the response, not an inference about a viewer:
    /// whatever the browser does with the state it gets, the turns that never
    /// became a state cannot be drawn by anybody. One turn per request is the
    /// only shape that costs nothing.
    #[test]
    fn a_batch_of_turns_answers_with_one_state_and_owes_the_rest() {
        let mut delivery = FrameDelivery::default();

        // The shape the browser is supposed to use.
        for _ in 0..5 {
            delivery.turns_simulated_without_a_frame(1);
        }
        assert_eq!(delivery.missed, 0, "one turn per response is one frame each");
        assert_eq!(delivery.autoplayed, 5);

        // The shape that lost them: ten turns, one state.
        delivery.turns_simulated_without_a_frame(10);
        assert_eq!(delivery.missed, 9, "nine of the ten turns had no state");
        assert_eq!(delivery.autoplayed, 15);

        // A request that ran out of game played nothing and owes nothing.
        delivery.turns_simulated_without_a_frame(0);
        assert_eq!(delivery.missed, 9);
        assert_eq!(delivery.autoplayed, 15);
    }

    /// `/status` has to be able to see auto-play, which it could not.
    ///
    /// `frames_missed` is built out of the gaps between a viewer's
    /// `/state?painted=` acknowledgements. A single-player page advances over
    /// `POST /autoplay` and sends none, and the exhibition stepper does not
    /// touch a game somebody is playing — so auto-play could drop nine turns
    /// in ten and every number on the audit still read clean. That is why this
    /// went unnoticed for as long as it did, so the audit is what gets the
    /// test: one turn per request must cost nothing, a batch must be charged
    /// for what it swallowed, and a retried request must not be charged twice.
    #[test]
    fn the_audit_can_see_the_turns_auto_play_never_shows() {
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
        assert!(!params.spectate, "auto-play is the human-game path");
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "single-player server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        let audit = |port| -> Value {
            serde_json::from_str(&http_get(port, "/status").expect("status")).expect("status JSON")
        };
        let ask = |turns: u32, id: &str| {
            json!({
                "turns": turns,
                "strategy": "basic",
                "request_id": id,
                "seed": 20_260_726,
                "server_instance": std::process::id(),
            })
            .to_string()
        };

        assert_eq!(
            audit(port)["autoplay_turns"],
            json!(0),
            "nothing has been auto-played yet, and that is a different claim from a clean run"
        );

        // The shape the browser uses: one turn, one state, one frame.
        for n in 1..=5 {
            let body = ask(1, &format!("viewer-1-autoplay-{n}"));
            let played: Value =
                serde_json::from_str(&http_post(port, "/autoplay", &body).expect("a response"))
                    .expect("response is JSON");
            assert_eq!(played["autoplayed"], json!(1), "request {n} played one turn");
        }
        let clean = audit(port);
        assert_eq!(clean["autoplay_turns"], json!(5));
        assert_eq!(
            clean["frames_missed"],
            json!(0),
            "five turns arrived as five states and none of them was owed a frame"
        );

        // The shape that lost them. Nine of these ten turns are simulated into
        // a state that is thrown away before it is ever serialised.
        let batch = ask(10, "viewer-1-autoplay-batch");
        let swallowed: Value =
            serde_json::from_str(&http_post(port, "/autoplay", &batch).expect("batch response"))
                .expect("batch response is JSON");
        assert_eq!(swallowed["autoplayed"], json!(10));
        let charged = audit(port);
        assert_eq!(charged["autoplay_turns"], json!(15));
        assert_eq!(
            charged["frames_missed"],
            json!(9),
            "a ten-turn batch answers with one state, so nine turns had none"
        );

        // A dropped response is replayed from `last_autoplay_request` without
        // simulating anything, so the retry arm must not be charged.
        let retry: Value =
            serde_json::from_str(&http_post(port, "/autoplay", &batch).expect("retry response"))
                .expect("retry response is JSON");
        assert_eq!(retry["turn"], swallowed["turn"], "the retry replayed the batch");
        let after_retry = audit(port);
        assert_eq!(
            after_retry["autoplay_turns"],
            json!(15),
            "the retry played no turns, so it owes none"
        );
        assert_eq!(after_retry["frames_missed"], json!(9));
    }

    /// Each viewer's misses are its own. One page catching every turn does not
    /// cover for another that is dropping them.
    #[test]
    fn misses_are_counted_against_the_viewer_that_missed_them() {
        let world = |turn| {
            Some(SpectatorFrame {
                seed: 41,
                turn,
                finished: false,
                sequence: turn as u64,
            })
        };
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
        let world = |turn| {
            Some(SpectatorFrame {
                seed: 41,
                turn,
                finished: false,
                sequence: turn as u64,
            })
        };
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
        poll(
            &mut delivery,
            Some(SpectatorFrame {
                seed: 42,
                turn: 40,
                finished: false,
                sequence: 40,
            }),
            beat,
        );
        poll(
            &mut delivery,
            Some(SpectatorFrame {
                seed: 42,
                turn: 40,
                finished: false,
                sequence: 40,
            }),
            beat,
        );
        poll(
            &mut delivery,
            Some(SpectatorFrame {
                seed: 42,
                turn: 41,
                finished: false,
                sequence: 41,
            }),
            beat,
        );
        assert_eq!(missed(&delivery), 3);
    }

    #[test]
    fn audit_counts_a_missing_player_frame_inside_one_turn() {
        let now = Instant::now();
        let first = SpectatorFrame {
            seed: 41,
            turn: 12,
            finished: false,
            sequence: 40,
        };
        let third = SpectatorFrame {
            sequence: 42,
            ..first
        };
        let mut delivery = FrameDelivery::default();
        delivery.frame_delivered("tab", first, now);
        delivery.viewer_request("tab", Some(first), now);
        delivery.frame_delivered("tab", third, now + Duration::from_millis(50));
        delivery.viewer_request("tab", Some(third), now + Duration::from_millis(50));
        assert_eq!(delivery.missed, 1, "sequence 41 was never painted");
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

    fn next_painted_state(viewer: &str, state: &Value) -> String {
        let seed = state["seed"].as_u64().expect("a world");
        let turn = state["turn"].as_u64().expect("a turn");
        let finished = u8::from(!state["winner"].is_null());
        let frame = state["frame_sequence"].as_u64().expect("a frame sequence");
        format!(
            "/state?painted={turn}&world={seed}&finished={finished}&frame={frame}\
             &viewer={viewer}&have={seed}:{turn}:{finished}:{frame}"
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
        http_post(port, "/pace", "{\"ms\":40}").expect("set the turn pace");

        // The browser's loop at its worst: one request in flight at a time,
        // and a paint that costs twice what the whole turn was budgeted. Only
        // that ratio is the scenario; the absolute numbers are as small as
        // stays clearly above scheduler jitter, because this loop is pure
        // wall-clock and once ran 24 x 250ms — the slowest test in the suite.
        let mut seen: Vec<u32> = Vec::new();
        let mut painted: Option<Value> = None;
        for _ in 0..24 {
            let target = match painted.as_ref() {
                None => "/state?painted=&viewer=slow-paint".to_string(),
                Some(state) => next_painted_state("slow-paint", state),
            };
            let body = http_get(port, &target).expect("a state to draw");
            let state: Value = serde_json::from_str(&body).expect("state is JSON");
            let turn = state["turn"].as_u64().expect("a turn") as u32;
            std::thread::sleep(Duration::from_millis(80)); // the paint
            seen.push(turn);
            painted = Some(state);
        }
        if let Some(state) = painted.as_ref() {
            http_get(port, &next_painted_state("slow-paint", state));
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

    /// A restart puts a new page in front of a new world, and that page's
    /// opening `/state?painted=` is the whole of how it gets there. It must
    /// not be made to wait on somebody else's paint.
    ///
    /// The stepper used to hold the frame gate across `wait_for_turn_frame`,
    /// and seating a viewer needs that same gate — so an arriving page was
    /// locked out for exactly as long as whoever was already watching took to
    /// draw. Measured on a recorded 74x46 six-player profile, that was 4.5s of
    /// veil on every restart against 0.05s once the wait moved outside the gate.
    /// Worse, the restarting page gave up at its own timeout and retried,
    /// seating itself as a viewer owed a frame it then never painted, and the
    /// successor stopped stepping altogether: 0 turns in 25 seconds.
    #[test]
    fn an_arriving_page_is_not_held_up_by_another_viewers_paint() {
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
        params.seed = 20_260_727;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "spectator server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }

        // Somebody is already watching, and drawing slowly. The stepper owes
        // this viewer every turn and will wait for it; the question is only
        // whether it waits with the door held shut behind it.
        // The paint below and the arrival bound in the loop are a matched
        // pair: a lock-out makes an arrival wait for the whole paint, so the
        // bound must sit well under the paint (2.4x here, as it always was)
        // while staying far above an uncontended read. Scaled from 1500/600ms
        // — which made this the suite's slowest test at 19s of wall clock —
        // to 600/250ms, which detects the same regression.
        let watcher = std::thread::spawn(move || {
            let mut painted: Option<Value> = None;
            for _ in 0..6 {
                let target = match painted.as_ref() {
                    None => "/state?painted=&viewer=watcher".to_string(),
                    Some(state) => next_painted_state("watcher", state),
                };
                let Some(body) = http_get(port, &target) else {
                    return;
                };
                let state: Value = serde_json::from_str(&body).expect("state is JSON");
                std::thread::sleep(Duration::from_millis(600)); // the paint
                painted = Some(state);
            }
        });
        std::thread::sleep(Duration::from_millis(300)); // let it take its seat

        // Each arrival is a distinct page, exactly as a reload or a restart is.
        // Asked repeatedly, because a lock-out is a race and one lucky read
        // proves nothing.
        for attempt in 1..=5 {
            let started = Instant::now();
            let body = http_get(port, &format!("/state?painted=&viewer=arriving-{attempt}"))
                .unwrap_or_else(|| panic!("arrival {attempt}: the opening state never arrived"));
            let elapsed = started.elapsed();
            let state: Value = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("arrival {attempt}: opening state is not JSON: {e}"));
            assert!(
                state["turn"].is_number(),
                "arrival {attempt}: the opening state carries no turn"
            );
            assert!(
                elapsed < Duration::from_millis(250),
                "arrival {attempt} waited {elapsed:?} for its first frame while another \
                 viewer was painting, and a restart shows a veil for every bit of that"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        http_post(port, "/pace", "{\"paused\":true}"); // stop stepping this game
        let _ = watcher.join();
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
        let frame = first["frame_sequence"].as_u64().expect("a frame sequence");
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
                &format!(
                    "/state?painted={turn}&world={seed}&finished=0&frame={frame}\
                     &viewer=paint-gate&have={seed}:{turn}:0:{frame}"
                ),
            )
            .expect("the next state"),
        )
        .expect("state is JSON");
        assert_eq!(next["turn"], json!(turn + 1));
        http_post(port, "/pace", "{\"paused\":true}");
    }

    /// The socket path, not only the helper that decides its policy: at Blitz
    /// each response is the state immediately after the seat named by the
    /// preceding response's `current` field acted. The sequence advances by
    /// one, player ids advance in the roster's Civ-style order, and any unit
    /// that changed position belongs to that acting seat.
    #[test]
    fn blitz_server_posts_each_players_completed_turn() {
        let port = exhibition(20_260_802);
        http_post(port, "/pace", "{\"paused\":true}").expect("pause before attaching");
        std::thread::sleep(Duration::from_millis(200));
        let mut state: Value = serde_json::from_str(
            &http_get(port, "/state?painted=&viewer=player-turns").expect("opening frame"),
        )
        .expect("state is JSON");
        let living: Vec<usize> = state["players"]
            .as_array()
            .expect("players")
            .iter()
            .filter(|player| player["alive"].as_bool().unwrap_or(false))
            .map(|player| player["id"].as_u64().expect("player id") as usize)
            .collect();
        assert!(living.len() >= 5, "the exhibition roster is too small: {living:?}");
        http_post(port, "/pace", "{\"ms\":500,\"paused\":false}")
            .expect("run at Blitz");

        let mut movement_seen = false;
        for _ in 0..living.len() * 2 {
            let acting = state["current"].as_u64().expect("acting player") as usize;
            let sequence = state["frame_sequence"].as_u64().expect("frame sequence");
            let positions = state["units"]
                .as_array()
                .expect("units")
                .iter()
                .filter_map(|unit| {
                    Some((
                        unit["id"].as_u64()?,
                        (
                            unit["owner"].as_u64()? as usize,
                            unit["pos"].clone(),
                        ),
                    ))
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            let next: Value = serde_json::from_str(
                &http_get(port, &next_painted_state("player-turns", &state))
                    .expect("the next player-turn frame"),
            )
            .expect("state is JSON");
            assert_eq!(
                next["frame_sequence"],
                json!(sequence + 1),
                "one Blitz seat must produce exactly one new frame"
            );
            let at = living.iter().position(|pid| *pid == acting).expect("living seat");
            let expected_next = living[(at + 1) % living.len()];
            assert_eq!(
                next["current"],
                json!(expected_next),
                "the frame after player {acting} skipped or reordered a seat"
            );
            for unit in next["units"].as_array().expect("units") {
                let Some(id) = unit["id"].as_u64() else { continue };
                if positions
                    .get(&id)
                    .is_some_and(|(_, before)| *before != unit["pos"])
                {
                    assert_eq!(
                        unit["owner"],
                        json!(acting),
                        "player {acting}'s frame moved another player's unit"
                    );
                    movement_seen = true;
                }
            }
            state = next;
        }
        http_post(port, "/pace", "{\"paused\":true}");
        assert!(movement_seen, "two complete rounds showed no unit movement");
        let status: Value = serde_json::from_str(&http_get(port, "/status").expect("status"))
            .expect("status is JSON");
        assert_eq!(status["frames_missed"], json!(0));
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

    /// The adjacency calculator rides the server so planning tools can ask a
    /// live game where districts belong. One city on request, every city by
    /// default, and every site carries the ledger that explains its figure.
    #[test]
    fn adjacency_calculator_is_served_per_city() {
        let port = exhibition(20_260_801);
        let deadline = Instant::now() + Duration::from_secs(60);
        let city = loop {
            assert!(Instant::now() < deadline, "no capital was ever founded");
            let state: Value =
                serde_json::from_str(&http_get(port, "/state").expect("state")).unwrap();
            if let Some(city) = state["cities"].as_array().and_then(|cities| cities.first()) {
                break city["id"].as_u64().unwrap();
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        http_post(port, "/pace", "{\"paused\":true}").expect("pause the exhibition");

        let body = http_get(port, &format!("/adjacency?city={city}")).expect("adjacency");
        let parsed: Value = serde_json::from_str(&body).expect("adjacency is JSON");
        let cities = parsed["cities"].as_array().expect("cities array");
        assert_eq!(cities.len(), 1, "?city narrows the document to one city");
        assert_eq!(cities[0]["id"].as_u64(), Some(city));
        let forecasts = cities[0]["forecasts"].as_array().expect("forecasts");
        assert!(!forecasts.is_empty(), "a capital has districts to plan");
        for forecast in forecasts {
            assert!(forecast["district"].is_string());
            assert!(forecast["family"].is_string());
            for site in forecast["sites"].as_array().expect("sites") {
                assert!(site["pos"].is_array());
                assert!(site["yields"].is_object());
                let mut ledger = 0.0;
                for source in site["sources"].as_array().expect("ledger") {
                    for yield_key in ["food", "production", "gold", "science", "culture", "faith"]
                    {
                        ledger += source["yields"][yield_key].as_f64().unwrap_or(0.0);
                    }
                }
                let mut total = 0.0;
                for yield_key in ["food", "production", "gold", "science", "culture", "faith"] {
                    total += site["yields"][yield_key].as_f64().unwrap_or(0.0);
                }
                assert!(
                    (ledger - total).abs() < 1e-9,
                    "served ledger must add up to the served yields"
                );
            }
        }

        let all: Value =
            serde_json::from_str(&http_get(port, "/adjacency").expect("all cities")).unwrap();
        assert!(
            !all["cities"].as_array().expect("cities").is_empty(),
            "the default document covers the world's cities"
        );
    }

    #[test]
    fn runtime_identity_is_a_small_successor_probe() {
        let port = exhibition(20_260_726);
        http_post(port, "/pace", "{\"paused\":true}").expect("pause the exhibition");
        let runtime_body = http_get(port, "/runtime").expect("runtime identity");
        let state_body = http_get(port, "/state").expect("full state");
        let runtime: Value = serde_json::from_str(&runtime_body).expect("runtime is JSON");
        let state: Value = serde_json::from_str(&state_body).expect("state is JSON");

        assert_eq!(runtime["server_instance"], state["server_instance"]);
        assert_eq!(runtime["seed"], state["seed"]);
        assert_eq!(runtime["commit"], state["server_commit"]);
        assert_eq!(runtime["commit_time"], state["server_commit_time"]);
        assert_eq!(runtime["built_at"], state["server_built_at"]);
        assert_eq!(runtime["artifact_bytes"], state["server_artifact_bytes"]);
        assert_eq!(runtime["artifact_kind"], state["server_artifact_kind"]);
        assert!(runtime["artifact_bytes"].as_u64().unwrap_or(0) > 0);
        // The supervisor's whole view of a pending restart. It rides here
        // rather than only on `/state` because a long AI turn is exactly when
        // a restart is asked for and exactly when `/state` cannot be built.
        assert!(runtime["supervisor_request"].is_null());
        assert_eq!(runtime["supervisor_request"], state["supervisor_request"]);
        assert!(runtime["next_game_settings"].is_null());
        assert!(
            runtime_body.len() < 320,
            "successor identity should stay tiny, got {} bytes",
            runtime_body.len()
        );
        assert!(
            runtime_body.len() * 10 < state_body.len(),
            "the identity probe should not resemble a full observation"
        );
    }

    #[test]
    fn viewer_marks_the_selected_revision_and_its_compact_ages_above_the_minimap() {
        assert!(EMBEDDED_INDEX.contains("id=\"buildmark\""));
        assert!(EMBEDDED_INDEX.contains("function updateBuildMarker(st = state)"));
        assert!(EMBEDDED_INDEX.contains("st?.server_commit_time"));
        assert!(EMBEDDED_INDEX.contains("st?.server_built_at"));
        assert!(EMBEDDED_INDEX.contains("st?.server_artifact_bytes"));
        assert!(EMBEDDED_INDEX.contains("st?.server_artifact_kind"));
        assert!(EMBEDDED_INDEX.contains("function formatBuildSize(bytes)"));
        assert!(EMBEDDED_INDEX.contains("(bytes / 1e6).toFixed(1)} MB"));
        assert!(EMBEDDED_INDEX.contains("(artifactSize ? ` · Build size ${artifactSize}` : \"\")"));
        assert!(EMBEDDED_INDEX.contains("commit.slice(0, 7)"));
        assert!(EMBEDDED_INDEX.contains("id=\"buildmark-commit\""));
        assert!(EMBEDDED_INDEX.contains(
            "https://github.com/MartinHalvorson/CIVVIS/commit/${encodeURIComponent(commit)}"
        ));
        assert!(EMBEDDED_INDEX.contains("View commit ${shortCommit} on GitHub"));
        // The age is one unit at one decimal ("1.5h", "2.3d"), never a
        // two-unit chain — the marker has to fit beside the minimap.
        assert!(EMBEDDED_INDEX.contains("if (totalMinutes < 60) return `${Math.floor(totalMinutes)}m`"));
        assert!(EMBEDDED_INDEX.contains("${Number.isInteger(tenths) ? tenths : tenths.toFixed(1)}${unit}"));
        assert!(EMBEDDED_INDEX.contains("Commit is ${formatBuildAge(commitDate)} old"));
        assert!(EMBEDDED_INDEX.contains("Build is ${formatBuildAge(buildDate)} old"));
        assert!(EMBEDDED_INDEX.contains(
            "#buildmark {\n    /* The authored World minimap owns the lower-right corner at z-index 6."
        ));
        assert!(EMBEDDED_INDEX.contains("position: fixed; z-index: 16;\n    right:"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn promoted_binary_name_carries_the_runtime_revision() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            super::promoted_binary_commit(&format!("civvis-{commit}")),
            Some(commit.to_owned())
        );
        assert_eq!(super::promoted_binary_commit("civvis-brief"), None);
        assert_eq!(super::promoted_binary_commit("civvis-zzzz"), None);
    }

    /// A result has to arrive with the window it promises, and that window has
    /// to be answerable.
    ///
    /// The countdown used to be armed on the stepper's *next* pass, so for a
    /// beat `/state` carried a winner and no `restart_in` at all and the
    /// result screen opened on "preparing the next world" before it started
    /// counting. That beat is the difference between the selected window to
    /// press "one more turn" and however much of it is left over.
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
        let configured: Value = serde_json::from_str(
            &http_post(port, "/pace", "{\"between_game_countdown_ms\":3000}")
                .expect("configure result countdown"),
        )
        .expect("countdown configuration is JSON");
        assert_eq!(configured["error"], Value::Null);
        assert_eq!(configured["between_game_countdown_ms"], json!(3_000));

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
        // Every state that carries a winner carries the selected countdown
        // with it, including its unrounded remainder for the page's own clock.
        assert_eq!(
            decided["restart_in"],
            json!(3),
            "a result was published without the selected three seconds"
        );
        assert_eq!(decided["between_game_countdown_ms"], json!(3_000));
        let restart_in_ms = decided["restart_in_ms"]
            .as_u64()
            .expect("the page receives an unrounded remainder");
        assert!(restart_in_ms > 0 && restart_in_ms <= 3_000);
        assert_eq!(
            restart_in_ms.div_ceil(1_000),
            decided["restart_in"].as_u64().unwrap(),
            "the precise and backwards-compatible countdowns describe one deadline"
        );

        let played_on: Value = serde_json::from_str(
            &http_post(
                port,
                "/play-on",
                "{\"mode\":\"until_next_victory\",\"paused\":true}",
            )
            .expect("look around"),
        )
        .expect("play-on answers JSON");
        assert!(played_on["error"].is_null());
        assert!(played_on["winner"].is_null(), "the world is live again");
        // The verdict survives the extension, and the countdown that was
        // running for it does not. It keeps the turn the result was reported
        // on: this game runs to its three-turn limit, so the score tiebreak is
        // dated on turn three even though the count that settled it was taken
        // on the wrap into a fourth turn nobody plays.
        assert_eq!(played_on["decided"]["turn"], decided["victory_turn"]);
        assert_eq!(
            played_on["decided"]["victory_type"],
            decided["victory_type"]
        );
        assert_eq!(
            played_on["decided"]["mode"],
            json!("until_next_victory")
        );
        assert!(played_on["restart_in"].is_null());
        assert!(played_on["restart_in_ms"].is_null());
        assert!(played_on["turn_limit"].is_null(), "there is no new cap");
        assert_eq!(played_on["max_turns"], json!(3), "setup stays intact");
        assert_eq!(played_on["seed"], decided["seed"], "the same world");
        assert_eq!(played_on["paused"], json!(true));
        assert_eq!(played_on["spectator_paused"], json!(true));

        // "Take a look around" means the final map really is held. A pause
        // posted separately from play-on could let the stepper claim one AI
        // turn in between the two requests.
        std::thread::sleep(Duration::from_millis(350));
        let held = read("/state");
        assert_eq!(held["seed"], decided["seed"]);
        assert_eq!(held["turn"], played_on["turn"]);
        assert!(held["winner"].is_null());
        assert_eq!(held["paused"], json!(true));
    }

    #[test]
    fn no_between_game_countdown_starts_the_successor_without_a_result_hold() {
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
        params.seed = 20_260_804;
        params.max_turns = 1;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let runtime = |port| -> Value {
            serde_json::from_str(&http_get(port, "/runtime").expect("runtime"))
                .expect("runtime is JSON")
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        let initial_seed = loop {
            if let Some(seed) = http_get(port, "/runtime")
                .and_then(|body| serde_json::from_str::<Value>(&body).ok())
                .and_then(|state| state["seed"].as_u64())
            {
                break seed;
            }
            assert!(Instant::now() < deadline, "spectator server never came up");
            std::thread::sleep(Duration::from_millis(25));
        };
        let configured: Value = serde_json::from_str(
            &http_post(
                port,
                "/pace",
                "{\"ms\":0,\"between_game_countdown_ms\":0}",
            )
            .expect("configure no result hold"),
        )
        .expect("countdown configuration is JSON");
        assert_eq!(configured["error"], Value::Null);
        assert_eq!(configured["between_game_countdown_ms"], json!(0));

        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let successor_seed = runtime(port)["seed"].as_u64().expect("runtime seed");
            if successor_seed != initial_seed {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the zero-second setting left the finished game on screen"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
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
        http_post(port, "/pace", "{\"ms\":30}").expect("set the turn pace");

        // The two run side by side for the same stretch of wall clock rather
        // than for the same number of polls, because the whole point is that
        // they read at different rates: one an order of magnitude slower than
        // the other, which is what a big map on a loaded machine looks like
        // next to a small one. Two seconds is enough window for the slow
        // viewer to cover several turns; the ratios are the scenario, and the
        // original 6s/400ms version cost 7s of pure wall clock.
        let until = Instant::now() + Duration::from_secs(2);
        let watch = |name: &'static str, paint: u64| {
            std::thread::spawn(move || {
                let mut seen: Vec<u32> = Vec::new();
                let mut painted: Option<Value> = None;
                while Instant::now() < until {
                    let target = match painted.as_ref() {
                        None => format!("/state?painted=&viewer={name}"),
                        Some(state) => next_painted_state(name, state),
                    };
                    let Some(body) = http_get(port, &target) else {
                        continue;
                    };
                    let state: Value = serde_json::from_str(&body).expect("state is JSON");
                    let turn = state["turn"].as_u64().expect("a turn") as u32;
                    std::thread::sleep(Duration::from_millis(paint)); // the paint
                    seen.push(turn);
                    painted = Some(state);
                }
                seen
            })
        };
        let slow = watch("slow", 200);
        let quick = watch("quick", 20);
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

        let changed = |mutate: &dyn Fn(&mut Value)| {
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
        let frame = now["frame_sequence"].as_u64().expect("a frame sequence");
        let holding = format!(
            "/state?painted={turn}&world={seed}&finished=0&frame={frame}\
             &viewer=one&have={seed}:{turn}:0:{frame}"
        );

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
        let frame = first["frame_sequence"].as_u64().expect("a frame sequence");
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
            "/state?painted={base}&world={seed}&finished=0&frame={frame}\
             &viewer=one&have={seed}:{base}:0:{frame}"
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
            "/state?painted={base}&world={seed}&finished=0&frame={frame}\
             &viewer=one&have={seed}:{}:0:{frame}",
            base + 9_000,
        ));
        assert!(stale["map"]["tiles"].is_array());
        assert!(stale["map"]["tiles_changed"].is_null());
    }

    #[test]
    fn a_page_that_cannot_boot_says_so_instead_of_showing_a_black_map() {
        // The failure this pins is invisible from outside the browser: the
        // shell paints, the title is right, the server is healthy, and the map
        // is simply never drawn. A page that is still retrying must say which
        // attempt it is on, and must take the notice down once a world arrives.
        let boot = EMBEDDED_INDEX
            .split_once("async function boot() {")
            .expect("browser boot function")
            .1
            .split_once("\nasync function send(action) {")
            .expect("end of browser boot function")
            .0;
        assert!(
            boot.contains("showBootRetrying(++bootAttempts);"),
            "a failed boot must report itself on screen, not only to the console"
        );
        assert!(
            boot.contains("if (bootAttempts) { bootAttempts = 0; clearBootRetrying(); }"),
            "a boot that finally succeeds must clear its own notice"
        );
        assert!(EMBEDDED_INDEX.contains("Connecting to the server — retrying (attempt ${attempt})"));
    }

    /// Both result paths use the selected setting: a human finale reads the
    /// control directly, while the exhibition paints the exact server value.
    #[test]
    fn the_page_counts_a_finale_down_from_the_selected_between_game_interval() {
        assert!(EMBEDDED_INDEX.contains(
            "finaleCountdownDeadline = Date.now() + betweenGameCountdownMs();"
        ));
        assert!(EMBEDDED_INDEX.contains("Number(st.restart_in_ms)"));
        assert!(EMBEDDED_INDEX.contains("between_game_countdown_ms"));
    }

    /// A full spectator state intentionally waits for either the next frame or
    /// a one-second cap. That transport cadence cannot also be the visible
    /// clock: boundary jitter made 10 linger for two seconds and skipped other
    /// numbers. The page paints from the precise server remainder between
    /// snapshots, while duplicate or delayed replies can only shorten its
    /// deadline.
    #[test]
    fn exhibition_countdown_paints_between_full_state_responses() {
        let sync = EMBEDDED_INDEX
            .split_once("function syncExhibitionCountdown(st) {")
            .expect("exhibition countdown synchronizer")
            .1
            .split_once("\n}\n\nfor (const gesture")
            .expect("end of exhibition countdown synchronizer")
            .0;
        assert!(sync.contains("Number(st.restart_in_ms)"));
        assert!(sync.contains("Math.min(exhibitionCountdownDeadline, candidateDeadline)"));
        assert!(sync.contains("setInterval(paintExhibitionCountdown, 100)"));
        assert!(EMBEDDED_INDEX.contains("Math.ceil(milliseconds / 1000)"));
        assert!(EMBEDDED_INDEX.contains("The next world is beginning"));
        assert!(!EMBEDDED_INDEX.contains("countdownTime.textContent = `${st.restart_in}s`;"));
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
            .split_once(
                "function render(st, recordChronicle = true, acceptingSupervisedSuccessor = false) {",
            )
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
            .find("paintedFrame = {seed:st.seed, turn:st.turn,")
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
        assert!(turn_plate.contains("<strong>${reportedTurn()}</strong>"));
        assert!(!victory_hud.contains("<strong>${reportedTurn()}</strong>"));

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
        assert!(player_hud
            .contains("p.player_username || p.ai_username || p.ai_name || \"AI player\""));
        // Civilization has absorbed the old Empire action. Watch as remains a
        // distinct, wider perspective control with breathing room before the
        // player identity it changes.
        assert!(player_hud.contains(
            "class=\"diplomacy-identity diplomacy-civ-link\" data-hud-col=\"civ\" data-hud-action=\"capital\""
        ));
        assert!(!player_hud.contains("class=\"empire-link\""));
        assert!(EMBEDDED_INDEX.contains("--hud-watch-column: 68px"));
        // Watch-as and the All button heading it share one fixed track, so they
        // are inset by the same single pixel. Four pixels of difference read as
        // a head overhanging its own column.
        assert!(EMBEDDED_INDEX.contains("width: calc(100% - 2px); height: 22px; margin: 0 1px;"));
        assert_eq!(EMBEDDED_INDEX.matches("width: calc(100% - 2px);").count(), 2,
            "the two controls in the Watch-as column are the same width at every screen size");
        // And a player with nothing behind them still wears a rating: the
        // 1500 every player starts from, marked provisional rather than
        // replaced by a dash that would read as "cannot be rated".
        assert!(player_hud.contains("const playerElo = eloKnown ? `${eloValue}` : \"—\";"));
        assert!(player_hud.contains("const playerEloDelta = signedEloDelta(playerHudEloDeltaValue(p));"));
        assert!(player_hud.contains("class=\"diplomacy-identity-field diplomacy-elo-delta\""));
        assert!(player_hud.contains("${eloProvisional ? \" provisional\" : \"\"}"));
        assert!(EMBEDDED_INDEX.contains(".diplomacy-elo-value.provisional {"));

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

        // And the page tells the server which exact frame it painted. The
        // sequence distinguishes several player-turn frames inside one turn,
        // both for the simulation gate and for the missed-frame audit.
        assert!(EMBEDDED_INDEX.contains("paintedFrame = {seed:st.seed, turn:st.turn,"));
        assert!(EMBEDDED_INDEX
            .contains("finished:st.winner !== null && st.winner !== undefined"));
        assert!(EMBEDDED_INDEX
            .contains("&frame=${paintedFrame.sequence}"));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/state\" + paintedQuery())"));
        // Two tabs are two promises, so a page says which one it is, and what
        // it holds is asked separately from what it drew — a state can arrive,
        // patch the tiles and still fail to paint.
        assert!(EMBEDDED_INDEX.contains("&viewer=${VIEWER_ID}"));
        assert!(EMBEDDED_INDEX
            .contains("&have=${tileStore.seed}:${tileStore.turn}:${tileStore.finished ? 1 : 0}:${tileStore.sequence}"));

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

    #[test]
    fn browser_takes_over_a_same_code_successor_without_reloading_itself() {
        // Every world is a new server process, so "the process changed" is not
        // a reason to replace the document — "the code changed" is. Replacing
        // it costs a full application boot, and that boot is what a viewer
        // sees as the exhibition dropping into a loading screen mid-match.
        assert!(EMBEDDED_INDEX.contains("function servesThisDocumentsCode(commit)"));
        assert!(EMBEDDED_INDEX.contains("commit !== \"unknown\" && commit === documentCommit"));
        assert!(EMBEDDED_INDEX.contains("function adoptSupervisedSuccessor(instance, seed)"));
        // Keeping the document is only half of it. Everything outside the
        // browser identifies the followed server by this tab's own URL, and
        // civvis-refresh.sh navigates a tab whose `instance=` disagrees — from
        // AppleScript, where no handoff can be staged. So an adoption says
        // where it went, without loading anything to say it.
        assert!(EMBEDDED_INDEX.contains("history.replaceState(history.state, \"\", here.toString());"));
        // The document's build is the one behind its *first* frame. Comparing
        // against the previous server instead would let a run of same-code
        // successors walk this page onto code it is not running.
        assert!(EMBEDDED_INDEX.contains(
            "if (documentCommit === null && typeof st.server_commit === \"string\" &&"
        ));

        // What an adoption forgets is exactly what belonged to the world being
        // left. A tile delta is keyed to its map, so `have=` must never name a
        // world the new server has never heard of.
        let adopt = EMBEDDED_INDEX
            .split_once("function adoptSupervisedSuccessor(instance, seed) {")
            .expect("the in-place adoption")
            .1
            .split_once("\n}")
            .expect("the end of the adoption")
            .0;
        for forgotten in [
            "tileStore = null;",
            "paintedFrame = null;",
            "lastSeed2 = null;",
            "specPending = null;",
            "specFetchGeneration++;",
            // The world being left most of all: `render` returns the moment it
            // decides to adopt, before it commits the state it was handed, so
            // a `state` kept here is a world that never advances and whose
            // seed re-triggers the same decision on every frame — a page
            // frozen on a dead map with its loop running flat out.
            "state = null;",
            "resetAnim();",
        ] {
            assert!(
                adopt.contains(forgotten),
                "adopting a successor must forget {forgotten}"
            );
        }

        // Both ways a supervised world can change under this page take the
        // in-place route: a replacement *process* (every game boundary) and a
        // new *seed* on the one already being watched (the in-process
        // successor, and the arm a page coming back from the dark lands on).
        assert_eq!(
            EMBEDDED_INDEX
                .matches("adoptSupervisedSuccessor(st.server_instance, st.seed);")
                .count(),
            2,
            "a changed process and a changed world are both adopted, not reloaded"
        );
        // And it is not a reload, however the page got here. It reads the URL
        // to rewrite it; nothing here navigates.
        for navigation in ["location.replace", "location.href =", "location.assign"] {
            assert!(
                !adopt.contains(navigation),
                "an adopted successor keeps this document, so it must not {navigation}"
            );
        }

        // The watch loop runs on requestAnimationFrame, which Chrome suspends
        // in a hidden, occluded or background tab. A page frozen there stops
        // noticing that the world it holds has ended, so something has to keep
        // its idea of the live server true on a clock rAF cannot stop.
        // Measured before this existed: 23s and 18s on a dead process across
        // two consecutive boundaries, ended only by an AppleScript navigation.
        assert!(EMBEDDED_INDEX.contains("async function followLiveServer()"));
        assert!(EMBEDDED_INDEX.contains("setInterval(followLiveServer, FOLLOW_POLL_MS);"));
        assert!(EMBEDDED_INDEX.contains("if (!document.hidden) followLiveServer();"));
        let follow = EMBEDDED_INDEX
            .split_once("async function followLiveServer() {")
            .expect("the follower")
            .1
            .split_once("\n}")
            .expect("the end of the follower")
            .0;
        // It asks the lock-free endpoint. A page that cannot paint must never
        // take the simulation mutex to find out what it is missing.
        assert!(follow.contains("fetchJSON(\"/runtime\""));
        assert!(
            !follow.contains("\"/state"),
            "a page that cannot paint must not poll /state"
        );
        // And it stays off the network entirely while the frame loop is alive,
        // because that loop finds a successor sooner and knows more about it.
        assert!(follow.contains("Date.now() - lastSpecFrameAt < FOLLOW_IDLE_MS"));
    }

    #[test]
    fn browser_keeps_ai_strategy_and_its_decision_factors_together() {
        // One civilization picker anchors the current plan, research, civics,
        // and the factors behind its decisions. A second, unrelated civ filter
        // would let a reader inspect a plan for one empire beside another's
        // evidence, so it is deliberately absent.
        assert!(EMBEDDED_INDEX.contains("<span>AI strategy</span>"));
        assert!(EMBEDDED_INDEX.contains("id=\"strategysec\""));
        assert!(EMBEDDED_INDEX.contains("id=\"strategyplayer\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"reasonsec\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"reasonplayer\""));
        assert!(EMBEDDED_INDEX.contains("id=\"reasonlevel\""));
        assert!(EMBEDDED_INDEX.contains("id=\"reasontopic\""));
        assert!(EMBEDDED_INDEX.contains("id=\"reasonmore\""));
        assert!(EMBEDDED_INDEX.contains("const player = strategyViewSeat();"));
        assert!(EMBEDDED_INDEX.contains("function showMoreReasoning()"));
        assert!(EMBEDDED_INDEX.contains("const REASON_KEEP = 6000;"));
        // The engine reassesses grand strategy more often than it changes it.
        // Preserve that activity, but call out only an actual doctrine
        // transition and name the prior doctrine for readers following why a
        // new decision was made.
        assert!(EMBEDDED_INDEX.contains("function strategyShiftOrigins(player)"));
        assert!(EMBEDDED_INDEX.contains("const match = /^Grand strategy:\\s*(.+)$/.exec("));
        assert!(EMBEDDED_INDEX.contains("strategyShiftFrom"));
        assert!(EMBEDDED_INDEX.contains("Shift from ${reasonEscape(thought.strategyShiftFrom)}"));
        assert!(EMBEDDED_INDEX.contains("saved[\"ai-reasoning\"]"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"all\">All topics</option>"));
        // Depth is a floor, not an equality — a decision read without the plan
        // it serves explains nothing — and the three rungs are the ones the
        // engine records at.
        assert!(EMBEDDED_INDEX.contains("const REASON_LEVEL_RANK = {strategy: 0, decision: 1, detail: 2};"));
        for level in ["strategy", "decision", "detail"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("<option value=\"{level}\"")),
                "the depth filter is missing {level}"
            );
        }
        // Every topic the engine can record is offered, under the same name.
        for topic in crate::reasoning::Topic::ALL {
            assert!(
                EMBEDDED_INDEX
                    .contains(&format!("[\"{}\", \"{}\"]", topic.as_str(), topic.label())),
                "the topic filter is missing {}",
                topic.as_str()
            );
        }
        // The log arrives as a delta on the turn's own fetch, not as part of
        // the multi-megabyte world observation.
        assert!(EMBEDDED_INDEX.contains("const thinking = `&think=${reasoningLog.cursor}`;"));
        assert!(EMBEDDED_INDEX.contains("function absorbReasoning(st)"));
        // A change of observed seat discards what the page holds, the same way
        // a change of world does. `reasoning_json` stops sending a rival's
        // thoughts the moment Watch as is entered, but the page had absorbed
        // them while it was above the world — so without this the redaction is
        // defeated by having watched first.
        assert!(EMBEDDED_INDEX.contains("const viewer = st.view_player ?? null;"));
        assert!(EMBEDDED_INDEX
            .contains("if (newWorld || reasoningLog.viewer !== viewer) {"));
        // A factor trail that no longer reaches the first decision says so.
        assert!(EMBEDDED_INDEX.contains("Earlier decision factors have been discarded"));
        // The single dossier has the spare height formerly allocated to a
        // separate reasoning card. Its trail scrolls inside the dossier while
        // the world chronicles keep their measured floors.
        assert!(EMBEDDED_INDEX.contains("#strategysec[open] { flex: 3 0 0; min-height: 460px; }"));
        assert!(EMBEDDED_INDEX.contains("#eventsec[open] { flex: 2 0 0; min-height: 302px; }"));
        assert!(EMBEDDED_INDEX.contains("#warsec[open] { flex: 2 0 0; min-height: 266px; }"));
        assert!(EMBEDDED_INDEX.contains(".reason-list { flex: 1 1 auto; min-height: 132px;"));
        // The compact filter language is shared with the two world chronicles;
        // only depth and topic remain because civilization is selected once.
        assert!(EMBEDDED_INDEX.contains(".log-filters { flex: 0 0 auto;"));
        assert!(EMBEDDED_INDEX.contains(".strategy-reason-filters { flex: 0 0 auto;"));
        assert!(!EMBEDDED_INDEX.contains(".reason-filter"));
        assert!(EMBEDDED_INDEX.contains("Recorded factors behind ${civ.civ}'s plan and decisions"));
    }

    /// The war log and the game event log are narrowed the same way.
    ///
    /// Both panels are chronicles of a whole world, and by the industrial era
    /// a spectator is reading thirty conflicts and sixty notices looking for
    /// one empire's story. The civ filter is the question actually being
    /// asked; each log carries exactly one more, the one its own shape needs.
    #[test]
    fn browser_lets_an_observer_narrow_the_two_chronicles() {
        for control in ["warplayer", "warstatus", "eventplayer", "eventkind"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("id=\"{control}\"")),
                "the log filters are missing {control}"
            );
        }
        // The two world chronicles name a seat with one vocabulary and correct
        // a selection that has left the list the same way, so a filter can
        // never strand a reader on an empty panel with no way back out of it.
        assert!(EMBEDDED_INDEX.contains("function syncCivFilterOptions(select, seats, allLabel, chosen)"));
        assert!(EMBEDDED_INDEX
            .contains("const kept = chosen !== \"all\" && !seats.includes(Number(chosen)) ? \"all\" : chosen;"));
        assert!(EMBEDDED_INDEX.contains("function syncWarOptions(wars)"));
        assert!(EMBEDDED_INDEX.contains("function syncEventOptions()"));
        // A war is filtered on the belligerents the panel itself would name,
        // city-states dragged in by a Suzerain included — not on the two
        // civilizations in the declaration.
        assert!(EMBEDDED_INDEX.contains("function warBelligerents(war)"));
        assert!(EMBEDDED_INDEX.contains("for (const party of (war.parties || [])) seats.add(party.player);"));
        // The war log's own filtering happens in the hook the panel already
        // reads through, so the chronicle's order is still never rewritten.
        let war_filter = EMBEDDED_INDEX
            .split("function warsForLog(wars)")
            .nth(1)
            .expect("the war log's filter")
            .split("function warLogSeats(wars)")
            .next()
            .unwrap();
        for status in ["ongoing", "ended"] {
            assert!(
                war_filter.contains(&format!("warFilters.status === \"{status}\"")),
                "the war status filter is missing {status}"
            );
            assert!(
                EMBEDDED_INDEX.contains(&format!("<option value=\"{status}\">")),
                "the war status filter offers no {status} option"
            );
        }
        assert!(!war_filter.contains("sort("));
        // Every category the engine can stamp on an event is gathered by one
        // of the kinds the filter offers. A category no kind claims would be
        // an entry the combined log shows and no narrower question can reach.
        let kinds = EMBEDDED_INDEX
            .split("const EVENT_KINDS = [")
            .nth(1)
            .expect("the event log's kinds")
            .split("];")
            .next()
            .unwrap();
        for category in crate::game::EVENT_CATEGORIES {
            assert!(
                kinds.contains(&format!("\"{category}\"")),
                "no event-log kind gathers the {category} category"
            );
            assert!(
                EMBEDDED_INDEX.contains(&format!("{category}: \"")),
                "the {category} category reaches the log with no icon of its own"
            );
        }
        // An entry is filtered on the civilizations it is *about*, recorded as
        // the log is written. Matching on the text would make "Rome" in
        // "Rome captured Antium from Egypt" hide the entry from Egypt.
        assert!(EMBEDDED_INDEX.contains("function eventSubjects(event)"));
        assert!(EMBEDDED_INDEX.contains(
            "!eventSubjects(event).includes(Number(eventFilters.player))"
        ));
        assert!(EMBEDDED_INDEX.contains("\"War\", [event.player, event.former]"));
        assert!(EMBEDDED_INDEX.contains("\"War\", [event.aggressor, event.defender]"));
        // An engine entry is attributed by the id the engine sends, not by
        // whichever seat the frame happened to be observed from: the
        // spectator's feed rotates through the seats between frames, so the
        // combined log's entries come from all of them.
        assert!(EMBEDDED_INDEX.contains("event.category, [event.player ?? eventViewPlayer(next)]"));
        // A dossier or chronicle choice is answered on the frame it is made
        // on, and survives the tab being closed.
        assert!(EMBEDDED_INDEX
            .contains("function applyLogFilter(filters, storageKey, field, value, listId, redraw)"));
        for key in [
            "civvis-ai-strategy-filters-v1",
            "civvis-war-filters-v1",
            "civvis-event-filters-v1",
        ] {
            assert!(EMBEDDED_INDEX.contains(key), "no stored preference for {key}");
        }
        // A narrowed panel says so rather than reading as a quiet world: "At
        // peace" under a filter would be a claim about the world made by a
        // panel that is only showing part of it.
        assert!(EMBEDDED_INDEX.contains("No conflict matches that filter."));
        assert!(EMBEDDED_INDEX.contains("Nothing in this log matches that filter."));
        assert!(EMBEDDED_INDEX.contains("`${wars.length} of ${rawWars.length}`"));
        assert!(EMBEDDED_INDEX.contains("`${events.length} of ${held.length}`"));
    }

    /// Every topic the reasoning log offers is one a game actually reaches.
    ///
    /// Ignored because it plays two hundred turns of a six-player world, which
    /// is minutes in a debug build. Run it with `cargo test --release --
    /// --ignored reasoning_reaches_every_topic` after adding a topic: a filter
    /// entry nothing ever writes to is a control that looks broken.
    ///
    /// Measured 2026-07-27 at turn 204 of seed 20260727: a median of 27
    /// thoughts a turn and a worst turn of 70, which is the few kilobytes per
    /// turn the delta on `/state` was designed around.
    #[test]
    #[ignore]
    fn reasoning_reaches_every_topic() {
        let mut params = watched_table();
        params.num_players = 6;
        params.width = 44;
        params.height = 26;
        params.num_city_states = 6;
        let mut session = Session::new(params);
        let mut seen: std::collections::BTreeMap<String, usize> = Default::default();
        let mut per_turn: Vec<usize> = Vec::new();
        let (mut cursor, mut turn, mut this_turn) = (0u64, session.game.turn, 0usize);
        for _ in 0..5_000 {
            session.step();
            let delta = session.reasoning_json(cursor);
            cursor = delta["cursor"].as_u64().unwrap();
            for thought in delta["thoughts"].as_array().unwrap() {
                *seen
                    .entry(thought["topic"].as_str().unwrap().to_string())
                    .or_default() += 1;
                this_turn += 1;
            }
            if session.game.turn != turn {
                per_turn.push(std::mem::take(&mut this_turn));
                turn = session.game.turn;
            }
            if session.game.winner.is_some() {
                break;
            }
        }
        per_turn.sort_unstable();
        let missing: Vec<&str> = crate::reasoning::Topic::ALL
            .iter()
            .map(|topic| topic.as_str())
            .filter(|topic| !seen.contains_key(*topic))
            .collect();
        assert!(
            missing.is_empty(),
            "reached turn {} and these topics never recorded anything: {missing:?} \
             (counts {seen:?})",
            session.game.turn
        );
        let median = per_turn[per_turn.len() / 2];
        assert!(
            median < 200,
            "a turn's reasoning delta has grown to {median} thoughts, which no longer \
             belongs on the per-turn `/state` fetch"
        );
    }

    /// A spectated table, which is the only kind that records its reasoning.
    fn watched_table() -> Params {
        let mut params = current();
        params.spectate = true;
        params.num_players = 3;
        params.seed = 20_260_727;
        params
    }

    #[test]
    fn a_watched_table_records_why_it_did_what_it_did() {
        let mut session = Session::new(watched_table());
        for _ in 0..24 {
            session.step();
        }
        let delta = session.reasoning_json(0);
        let thoughts = delta["thoughts"].as_array().expect("a reasoning delta");
        assert!(
            !thoughts.is_empty(),
            "twenty-four turns of a watched table produced no reasoning at all"
        );
        // Every thought says who thought it, when, and at what depth, because
        // those three are exactly what the observer's filters run on.
        for thought in thoughts {
            assert!(thought["turn"].is_u64());
            assert!(thought["player"].is_u64());
            assert!(!thought["headline"].as_str().unwrap_or("").is_empty());
            assert!(matches!(
                thought["level"].as_str(),
                Some("strategy" | "decision" | "detail")
            ));
            assert!(crate::reasoning::Topic::ALL
                .iter()
                .any(|topic| topic.as_str() == thought["topic"].as_str().unwrap_or("")));
        }
        // More than one civilization is thinking, and the record is one
        // ordered account rather than one log per seat.
        let seats: std::collections::BTreeSet<u64> = thoughts
            .iter()
            .filter_map(|thought| thought["player"].as_u64())
            .collect();
        assert!(seats.len() > 1, "only one seat was recorded: {seats:?}");
        let ids: Vec<u64> = thoughts
            .iter()
            .filter_map(|thought| thought["id"].as_u64())
            .collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "ids are not ordered");
        // The plan itself has to be in there — it is what every other line is
        // an instance of.
        assert!(
            thoughts.iter().any(|thought| {
                thought["level"] == json!("strategy")
                    && thought["headline"]
                        .as_str()
                        .is_some_and(|line| line.starts_with("Grand strategy: "))
            }),
            "no civilization reported its grand strategy"
        );
    }

    #[test]
    fn a_reasoning_cursor_is_answered_with_only_the_new_thoughts() {
        let mut session = Session::new(watched_table());
        for _ in 0..12 {
            session.step();
        }
        let first = session.reasoning_json(0);
        let cursor = first["cursor"].as_u64().expect("a cursor");
        assert!(cursor > 0);
        assert_eq!(first["reset"], json!(false));
        // Nothing has happened since, so nothing comes back. This is the
        // property that keeps the delta off the per-turn `/state` budget.
        let idle = session.reasoning_json(cursor);
        assert!(idle["thoughts"].as_array().unwrap().is_empty());
        for _ in 0..6 {
            session.step();
        }
        let next = session.reasoning_json(cursor);
        let fresh = next["thoughts"].as_array().unwrap();
        assert!(!fresh.is_empty());
        assert!(
            fresh.iter().all(|thought| thought["id"].as_u64().unwrap() > cursor),
            "a cursor was answered with thoughts it already held"
        );
        // A cursor from a world this log is not — a tab that outlived the
        // previous game — is told to discard what it holds.
        let stale = session.reasoning_json(u64::MAX);
        assert_eq!(stale["reset"], json!(true));
        assert!(!stale["thoughts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn watching_as_one_civilization_shows_only_that_civilizations_reasoning() {
        let mut session = Session::new(watched_table());
        for _ in 0..18 {
            session.step();
        }
        let everyone = session.reasoning_json(0);
        let seats: std::collections::BTreeSet<u64> = everyone["thoughts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|thought| thought["player"].as_u64())
            .collect();
        assert!(seats.len() > 1);
        // Sitting behind one civilization's fog means seeing what it can see,
        // and nobody can read a rival's mind. The redaction is on the wire.
        session.set_view_player(Some(1)).expect("a major seat");
        let seated = session.reasoning_json(0);
        let thoughts = seated["thoughts"].as_array().unwrap();
        assert!(!thoughts.is_empty());
        assert!(
            thoughts.iter().all(|thought| thought["player"] == json!(1)),
            "another civilization's reasoning reached a fogged view"
        );
    }

    #[test]
    fn an_unwatched_game_records_nothing() {
        // A headless game has nobody reading the log, and the cost of building
        // one is the cost of every `format!` in the agent. Off is the default
        // and this is what pins it.
        let mut session = Session::new(current());
        for _ in 0..8 {
            session.step();
        }
        let delta = session.reasoning_json(0);
        assert!(delta["thoughts"].as_array().unwrap().is_empty());
        assert_eq!(delta["cursor"], json!(0));
    }

    #[test]
    fn the_state_document_carries_reasoning_only_when_a_page_asks_for_it() {
        // `/state` is close to a megabyte on a standard map and a watching
        // page fetches one per turn. The reasoning rides along only for a
        // client that named a cursor.
        let mut session = Session::new(watched_table());
        for _ in 0..6 {
            session.step();
        }
        assert!(session.state().get("ai_reasoning").is_none());
        let mut with_reasoning = session.state();
        with_reasoning["ai_reasoning"] = session.reasoning_json(0);
        assert!(with_reasoning["ai_reasoning"]["thoughts"]
            .as_array()
            .is_some_and(|thoughts| !thoughts.is_empty()));
    }

    #[test]
    fn a_watched_simultaneous_game_steps_a_whole_turn() {
        // The spectator server plays the simultaneous regime as one whole
        // planned turn per step, and the state document names the regime and
        // carries its census so the viewer can read both.
        let mut params = watched_table();
        params.turn_structure = TurnStructure::Simultaneous;
        let mut session = Session::new(params);
        let turn_before = session.game.turn;
        session.step();
        assert_eq!(
            session.game.turn,
            turn_before + 1,
            "one step of a simultaneous game is one whole game turn"
        );
        let state = session.state();
        assert_eq!(state["turn_structure"], json!("simultaneous"));
        assert!(state["simultaneous"]["planned"].as_u64().unwrap() > 0);
    }

    #[test]
    fn a_sequential_state_names_its_turn_structure_and_no_census() {
        let session = Session::new(watched_table());
        let state = session.state();
        assert_eq!(state["turn_structure"], json!("sequential"));
        assert!(state.get("simultaneous").is_none());
    }

    #[test]
    fn new_game_carries_the_turn_structure_for_a_watched_table_only() {
        let mut spectated = current();
        spectated.spectate = true;
        let asked = new_game_params(&spectated, &json!({"turn_structure": "simultaneous"}));
        assert_eq!(asked.turn_structure, TurnStructure::Simultaneous);
        // An unknown id leaves the setting where it was, like every rung.
        let nonsense = new_game_params(&asked, &json!({"turn_structure": "psychic"}));
        assert_eq!(nonsense.turn_structure, TurnStructure::Simultaneous);
        let back = new_game_params(&asked, &json!({"turn_structure": "sequential"}));
        assert_eq!(back.turn_structure, TurnStructure::Sequential);
        // A game with a human seat is sequential by construction: the rest
        // of such a request lands, the regime does not.
        let played = new_game_params(&current(), &json!({"turn_structure": "simultaneous"}));
        assert_eq!(played.turn_structure, TurnStructure::Sequential);
    }

    fn current() -> Params {
        Params {
            map_topology: MapTopology::Flat,
            map_poles: MapPoles::Poles,
            base_ruleset: BaseRuleset::Civ6,
            start_era: 0,
            future_era: FutureEra::Classic,
            turn_structure: TurnStructure::Sequential,
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
            leader_pool: LeaderPool::Civ6,
            civs: Vec::new(),
            supervised: false,
            league_dir: None,
            league_record: false,
            force_strategy: None,
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
        let mut params = current();
        params.leader_pool = LeaderPool::ExpandedHistorical;
        let mut session = Session::new(params);
        for _ in 0..3 {
            session.act(&json!({"type": "end_turn"}));
        }
        let turn = session.game.turn;
        assert!(turn > 1, "the game should have advanced");

        let round_tripped: Game =
            serde_json::from_value(serde_json::to_value(&session.game).unwrap()).unwrap();
        assert_eq!(round_tripped.turn, turn);
        assert_eq!(round_tripped.seed, session.game.seed);
        assert_eq!(round_tripped.leader_pool, LeaderPool::ExpandedHistorical);

        let mut restored = Session::from_game(session.params.clone(), round_tripped);
        assert_eq!(restored.game.turn, turn);
        assert_eq!(restored.params.leader_pool, LeaderPool::ExpandedHistorical);
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

        // The browser's custom cap is shared by single player and the
        // exhibition. Invalid values must not wrap the engine's u32 field or
        // turn a playable match into a zero-turn result.
        for invalid in [0, u64::from(u32::MAX) + 1] {
            let ignored = new_game_params(
                &current(),
                &json!({"game_speed": "quick", "max_turns": invalid}),
            );
            assert_eq!(ignored.max_turns, GameSpeed::Quick.turn_limit());
        }
    }

    /// The shared lobby fields are not spectator-only metadata. They shape a
    /// human world in exactly the same way as an AI exhibition, and the
    /// normalized state is what the native supervisor and wasm router both
    /// hand back to the browser after staging.
    #[test]
    fn shared_world_setup_round_trips_city_states_turn_cap_and_seed() {
        let request = json!({
            "num_players": 6,
            "num_city_states": 2,
            "game_speed": "epic",
            "max_turns": 1_234,
            "seed": 8_765_432,
            "spectate": false,
        });
        let params = new_game_params(&current(), &request);
        assert_eq!(params.num_city_states, 2);
        assert_eq!(params.max_turns, 1_234);
        assert_eq!(params.seed, 8_765_432);
        assert!(!params.spectate);

        let settings = simulation_settings(&params);
        assert_eq!(settings["city_states"], json!(2));
        assert_eq!(settings["turns"], json!(1_234));
        assert_eq!(settings["seed"], json!(8_765_432));

        // Staging retains world settings but leaves the active world's mode
        // alone; an automatic successor may never silently turn into a game
        // nobody is controlling.
        let staged = staged_next_game_params(&current(), &request);
        assert!(staged.spectate == current().spectate);
        assert_eq!(simulation_settings(&staged), settings);
    }

    #[test]
    fn planetary_future_era_world_is_supported() {
        let params = new_game_params(
            &current(),
            &json!({
                "map_topology": "planet",
                "map_script": "true_start_earth",
                "start_era": "future",
            }),
        );
        assert_eq!(params.map_topology, MapTopology::Planet);
        assert_eq!(params.map_script, MapScript::TrueStartEarth);
        assert_eq!(params.start_era, start_era_from_id("future").unwrap());

        let session = Session::new(params);
        assert_eq!(session.game.map_script, MapScript::TrueStartEarth);
        assert_eq!(session.game.start_era, start_era_from_id("future").unwrap());
        assert_eq!(session.game.world_era, start_era_from_id("future").unwrap());
    }

    /// The first question the lobby asks. One ruleset is modeled, so the
    /// setting has exactly one legal answer, and an id from some other game
    /// leaves it on Civilization VI rather than being taken at face value.
    #[test]
    fn the_base_ruleset_setting_accepts_only_the_game_this_models() {
        let stock = current();
        assert_eq!(stock.base_ruleset, BaseRuleset::Civ6);
        assert_eq!(
            new_game_params(&stock, &json!({"base_ruleset": "civ5"})).base_ruleset,
            BaseRuleset::Civ6
        );
        let asked = new_game_params(&stock, &json!({"base_ruleset": "civ6"}));
        assert_eq!(asked.base_ruleset, BaseRuleset::Civ6);
        assert_eq!(simulation_settings(&asked)["base_ruleset"], "civ6");
        assert_eq!(Session::new(asked).game.base_ruleset, BaseRuleset::Civ6);
    }

    /// A rung that is declared but not built is refused, not substituted: a
    /// lobby that asks for the Stone Age and is quietly handed the Ancient era
    /// has been lied to about what it is about to play. An unbuilt rung and an
    /// unknown one are answered the same way, because in both cases the honest
    /// reply is that the setting did not move.
    #[test]
    fn a_start_era_nobody_has_built_is_refused_rather_than_substituted() {
        let stock = current();
        assert_eq!(stock.start_era, 0);
        assert_eq!(simulation_settings(&stock)["start_era"], "ancient");
        // The eon the ladder used to hang off is gone from the wire entirely.
        assert!(simulation_settings(&stock).get("eon").is_none());

        let medieval = new_game_params(&stock, &json!({"start_era": "medieval"}));
        assert_eq!(medieval.start_era, start_era_from_id("medieval").unwrap());
        assert_eq!(simulation_settings(&medieval)["start_era"], "medieval");

        let future = new_game_params(&medieval, &json!({"start_era": "future"}));
        assert_eq!(future.start_era, start_era_from_id("future").unwrap());
        assert_eq!(simulation_settings(&future)["start_era"], "future");

        // The Stone Age is on the ladder but has no tree behind it; the rest
        // are not rungs at all. Every one of them leaves the setting alone.
        for refused in ["stone_age", "dinosaur", "holocene", ""] {
            let asked = new_game_params(&medieval, &json!({"start_era": refused}));
            assert_eq!(
                asked.start_era, medieval.start_era,
                "{refused:?} moved the start era"
            );
            assert_eq!(simulation_settings(&asked)["start_era"], "medieval");
        }
    }

    /// The other end of the game is asked for the same way, is refused the
    /// same way, and — unlike the start era — changes the rules rather than
    /// the world, so the ruleset the session ends up holding is the test.
    #[test]
    fn the_lobby_can_ask_for_the_modified_future_era() {
        let stock = current();
        assert_eq!(stock.future_era, FutureEra::Classic);
        assert_eq!(simulation_settings(&stock)["future_era"], "classic");

        let modified = new_game_params(&stock, &json!({"future_era": "modified"}));
        assert_eq!(modified.future_era, future_era_from_id("modified").unwrap());
        assert_eq!(simulation_settings(&modified)["future_era"], "modified");

        for refused in ["martin", "gathering_storm", ""] {
            let asked = new_game_params(&modified, &json!({"future_era": refused}));
            assert_eq!(
                asked.future_era, modified.future_era,
                "{refused:?} moved the Future Era"
            );
            assert_eq!(simulation_settings(&asked)["future_era"], "modified");
        }

        // And the setting reaches the rules: the Moon has ore on it and there
        // is something to throw the ore with.
        let session = Session::new(modified);
        assert_eq!(session.game.future_era, FutureEra::Modified);
        assert!(session.game.rules.projects.contains_key("mass_driver"));
        assert!(!session.game.moon_deposits.is_empty());

        let classic = Session::new(new_game_params(&stock, &json!({})));
        assert!(!classic.game.rules.projects.contains_key("mass_driver"));
        assert!(classic.game.moon_deposits.is_empty());
    }

    /// The setting has to reach the world, survive being read back off it, and
    /// be what the lobby is offered again next time.
    #[test]
    fn a_started_game_opens_in_the_era_the_lobby_asked_for() {
        let asked = new_game_params(&current(), &json!({"start_era": "medieval"}));
        let era = start_era_from_id("medieval").unwrap();
        let session = Session::new(asked);
        assert_eq!(session.game.start_era, era);
        assert_eq!(session.game.world_era, era);
        // Everyone on the board opens with the earlier eras researched.
        assert!(session
            .game
            .players
            .iter()
            .filter(|player| !player.is_barbarian)
            .all(|player| !player.techs.is_empty()));

        // A world restored from that game offers its own setup back, not the
        // default one — this is the lobby's only source of truth for what is
        // on screen.
        let params = current();
        let restored = Session::from_game(params, session.game.clone());
        assert_eq!(restored.params.start_era, era);
        assert_eq!(restored.params.base_ruleset, BaseRuleset::Civ6);
    }

    /// A named scenario belongs in the public setup only after it owns the
    /// country roster, geographic state, and launch path it promises. The
    /// ordinary Earth and Future-era controls remain usable independently.
    #[test]
    fn browser_does_not_offer_the_incomplete_earth_today_preset() {
        for piece in [
            "id=\"scenario\"",
            "<option value=\"earth_today\">Earth Today</option>",
            "const GAME_SCENARIOS = Object.freeze({",
            "function applySelectedScenario()",
            "function syncScenarioFromSettings()",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(piece),
                "the incomplete Earth Today preset is still public through {piece}"
            );
        }
        assert!(EMBEDDED_INDEX.contains(
            "[\"gamemode\", true], [\"np\", true], [\"mapshape\", true], [\"maptype\", true],"
        ));
    }

    #[test]
    fn leader_pool_defaults_to_civ6_and_normalizes_legacy_expanded_to_historical() {
        let stock = current();
        assert_eq!(stock.leader_pool, LeaderPool::Civ6);

        let ignored = new_game_params(&stock, &json!({"leader_pool": "unknown"}));
        assert_eq!(ignored.leader_pool, LeaderPool::Civ6);

        let historical = new_game_params(&stock, &json!({"leader_pool": "historical"}));
        assert_eq!(historical.leader_pool, LeaderPool::ExpandedHistorical);
        let session = Session::new(historical);
        assert_eq!(session.game.leader_pool, LeaderPool::ExpandedHistorical);
        assert_eq!(session.state()["leader_pool"], "historical");

        let legacy = new_game_params(&stock, &json!({"leader_pool": "expanded"}));
        assert_eq!(legacy.leader_pool, LeaderPool::ExpandedHistorical);
        let unavailable_today = new_game_params(&stock, &json!({"leader_pool": "today"}));
        assert_eq!(unavailable_today.leader_pool, LeaderPool::Civ6);

        let stock_session = Session::new(stock);
        assert!(stock_session
            .game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .all(|player| CIV6_LEADER_POOL.contains(&player.civ.as_str())));
    }

    #[test]
    fn leader_choices_must_belong_to_the_selected_available_tier() {
        let historical = new_game_params(
            &current(),
            &json!({
                "leader_pool": "historical",
                "civs": ["Denmark", "Rome", "Romania"],
            }),
        );
        assert_eq!(historical.civs, vec!["Denmark"]);

        let civ6 = new_game_params(
            &current(),
            &json!({
                "leader_pool": "civ6",
                "civs": ["Denmark", "Rome"],
            }),
        );
        assert_eq!(civ6.civs, vec!["Rome"]);
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

        // Spectator state exposes every major; an interactive observer still
        // withholds an unmet rival's annotations under the normal fog rule.
        let mut visible_params = params.clone();
        visible_params.spectate = true;
        let session = Session::new(visible_params);
        assert_eq!(session.game.victory_conditions, params.victory_conditions);
        let state = session.state();
        assert_eq!(state["victory_conditions"], disabled);
        for player in state["players"]
            .as_array()
            .expect("state has players")
            .iter()
            .filter(|player| {
                player["is_minor"] != json!(true) && player["is_barbarian"] != json!(true)
            })
        {
            assert_eq!(player["odds_start"], json!(0.0));
            assert_eq!(player["odds_now"], json!(0.0));
        }
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

    /// Map search is deliberately a client-side read of the observation the
    /// browser already owns. It must use exact sight rather than remembered
    /// tiles, understand the named things that can occupy a tile, and paint
    /// the same result on both supported map projections.
    #[test]
    fn browser_search_lights_every_matching_visible_tile() {
        let sidebar = EMBEDDED_INDEX
            .split_once("<div id=\"side\">")
            .expect("left command deck")
            .1
            .split_once("<div id=\"buildmark\"")
            .expect("end of left command deck")
            .0;
        let lenses_at = sidebar
            .find("id=\"map-lenses\"")
            .expect("map lens menu in the deck");
        let search_at = sidebar
            .find("id=\"map-search\"")
            .expect("map search control in the deck");
        let shortcuts_at = sidebar
            .find("<summary>Keyboard shortcuts</summary>")
            .expect("keyboard shortcuts");
        assert!(
            lenses_at < search_at && search_at < shortcuts_at,
            "map lenses should sit above visible-tile search, immediately before keyboard shortcuts"
        );
        assert!(sidebar.contains("id=\"map-search-input\" type=\"search\""));
        assert!(sidebar.contains("id=\"map-search-civ\""));
        assert!(sidebar.contains("aria-live=\"polite\""));
        assert!(sidebar.contains(
            "<details id=\"maplensessec\" data-section=\"map-lenses\">"
        ));
        // The search is a deck panel in the fixed lower-left stack, not a
        // pill floating over the map: exactly one #map-search rule, the
        // deck's, and no absolutely positioned leftover from the dock era.
        assert!(EMBEDDED_INDEX.contains(
            "#map-search {\n    min-width: 0; padding: 6px 7px;"
        ));
        assert!(
            !EMBEDDED_INDEX.contains("#map-search {\n    position: absolute;"),
            "the visible-tile search must not float over the map"
        );

        let matcher = EMBEDDED_INDEX
            .split_once("function computeMapSearchMatches(st, query, civFilter = mapSearchCivId) {")
            .expect("map search matcher")
            .1
            .split_once("\nfunction mapSearchMatches() {")
            .expect("end of map search matcher")
            .0;
        assert!(matcher.contains("new Set((st.visible || []).map(key))"));
        assert!(
            !matcher.contains("turn_visible"),
            "search must not reveal remembered tiles outside exact sight"
        );
        for field in [
            "tile.terrain",
            "tile.feature",
            "tile.resource",
            "tile.improvement",
            "tile.district",
            "tile.planned_district",
            "tile.wonder",
            "st.cities",
            "st.units",
            "st.camps",
        ] {
            assert!(matcher.contains(field), "map search ignores {field}");
        }
        assert!(matcher.contains("RULES?.districts?.[district]?.replaces"));
        assert!(matcher.contains("if (selectedCiv && String(owner) !== selectedCiv) return;"));
        assert!(matcher.contains("if (!needle || mapSearchTextMatches(values, needle)) matches.add(at);"));
        assert!(EMBEDDED_INDEX.contains("function mapSearchVisibleCivilizations(st)"));
        assert!(EMBEDDED_INDEX.contains(
            "player && !player.is_barbarian && player.civ && owners.has(String(player.id))"
        ));
        assert!(EMBEDDED_INDEX.contains("if (Array.isArray(pos) && visible.has(key(pos))"));
        let civilization_sync = EMBEDDED_INDEX
            .split_once("function syncMapSearchCivilizations(st = state) {")
            .expect("civilization-filter synchronizer")
            .1
            .split_once("\nfunction computeMapSearchMatches(")
            .expect("end of civilization-filter synchronizer")
            .0;
        assert!(civilization_sync.contains(
            "const optionsChanged = !mapSearchCivilizationOptions(select, entries);"
        ));
        assert!(civilization_sync.contains(
            "if (optionsChanged && document.activeElement === select) return false;"
        ));
        assert!(civilization_sync.contains(
            "if (optionsChanged) {\n    select.replaceChildren(...entries.map"
        ));
        assert_eq!(
            civilization_sync.matches("select.replaceChildren").count(),
            1,
            "the native civilization picker must stay intact when the visible roster is unchanged"
        );
        assert!(EMBEDDED_INDEX.contains(
            "mapSearchCiv.addEventListener(\"blur\", () => {\n  // Apply a roster change held back while the native picker was expanded.\n  if (!syncMapSearchCivilizations()) return;"
        ));
        assert!(EMBEDDED_INDEX.contains("syncMapSearchCivilizations(st);"));
        assert!(EMBEDDED_INDEX.contains("drawFlatMapSearchHighlights(tiles);"));
        assert!(EMBEDDED_INDEX.contains("drawPlanetMapSearchHighlights(cells);"));
        assert!(EMBEDDED_INDEX.contains(
            "TMAP = new Map(st.map.tiles.map(t => [key(t.pos), t]));\n  syncMapSearchCivilizations(st);\n  syncMapSearchStatus();"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const selected = mapSearchCivId && select?.selectedOptions?.[0]?.textContent;"
        ));
        let lens_setter = EMBEDDED_INDEX
            .split_once("function setMapLens(next) {")
            .expect("lens setter")
            .1
            .split_once("\nfunction modeline() {")
            .expect("end of lens setter")
            .0;
        assert!(
            !lens_setter.contains("mapSearch"),
            "a lens must not clear an active visible-tile search"
        );
    }

    /// The omniscient spectator has no seat: `state.player` merely follows
    /// the acting major, so a district lens keyed to it flashed a different
    /// empire's information every simulated turn — and reset itself whenever
    /// the acting civilization replaced the selected family with a unique.
    /// Above the world the lens must instead hold still and show everything.
    #[test]
    fn browser_district_lens_holds_still_above_the_world() {
        // One switch, read everywhere the lens would otherwise follow the
        // acting player.
        assert!(EMBEDDED_INDEX.contains("function districtLensOmniscient() {"));
        assert!(EMBEDDED_INDEX
            .contains("return !!state?.spectate && !Number.isInteger(state?.view_player);"));
        // The lens list offers the stable base families and signs itself with
        // a constant name, so nothing rebuilds — or resets the active lens —
        // as turns rotate.
        assert!(EMBEDDED_INDEX
            .contains(".filter(district => !RULES.districts[district]?.unique_to)"));
        assert!(EMBEDDED_INDEX.contains("? \"omniscient\" : state?.players?.[viewer]?.civ || \"\";"));
        // Every empire's ground answers at once: existing districts match by
        // family and price themselves by their own rules, and candidate plots
        // are forecast for each empire with the district its civilization
        // actually builds.
        assert!(EMBEDDED_INDEX.contains("function districtLensCivVariant(family, civ) {"));
        assert!(EMBEDDED_INDEX.contains("if (!districtLensIsFamily(tile.district, district)) continue;"));
        assert!(EMBEDDED_INDEX
            .contains("? districtLensCivVariant(district, state.players?.[city.owner]?.civ)"));
        // The one observed seat's private reads — its laboratory and its
        // policy cards — are skipped above the world instead of flickering
        // through whichever empire happens to be acting.
        assert!(EMBEDDED_INDEX
            .contains("return districtLensOmniscient() ? null : new Set(state?.me?.techs || []);"));
        assert!(EMBEDDED_INDEX
            .contains("if (districtLensOmniscient() || owner !== districtLensViewer()) return 0;"));
        // Districts without adjacency rules — the Aerodrome family and
        // friends, exactly as in Civilization VI — say so in the modeline
        // rather than reading as a broken lens.
        assert!(EMBEDDED_INDEX.contains("function districtLensFamilyHasAdjacency(district) {"));
        assert!(EMBEDDED_INDEX
            .contains("no adjacency bonuses in Civ VI — showing legal sites"));
    }

    /// Every lobby setting is answered before a game starts: each select
    /// offers only real values, and a victory condition is the two states a
    /// checkbox knows. Nothing in the panel stands in for a decision that has
    /// not been made, and nothing rolls one on somebody's behalf.
    #[test]
    fn every_game_setting_is_answered_before_a_game_starts() {
        for setting in [
            "baseruleset",
            "gamemode",
            "startera",
            "futureera",
            "leaderpool",
            "leaderselection",
            "leader",
            "difficulty",
            "mapshape",
            "maptype",
            "mappoles",
            "np",
            "teams",
            "gamespeed",
        ] {
            let select = format!("id=\"{setting}\"");
            let at = EMBEDDED_INDEX
                .find(&select)
                .unwrap_or_else(|| panic!("browser setup is missing the {setting} select"));
            let tail = &EMBEDDED_INDEX[at..];
            let end = tail.find("</select>").expect("unterminated select");
            assert!(
                !tail[..end].contains("?????"),
                "the {setting} setting still offers a non-answer"
            );
        }
        for input in ["mapseed"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("id=\"{input}\" type=\"number\"")),
                "browser setup is missing the {input} numeric input"
            );
        }
        assert!(
            !EMBEDDED_INDEX.contains("id=\"citystates\""),
            "city-state count should come from the selected map size"
        );
        for victory in ["science", "culture", "religious", "diplomatic", "domination", "score"] {
            assert!(EMBEDDED_INDEX.contains(&format!(
                "id=\"victory-{victory}\" checked>"
            )));
        }
        // The lobby reads its own controls once, with no resolving pass and no
        // remembered marks between the panel and the payload.
        assert!(EMBEDDED_INDEX.contains("const payload = selectedSimulationSettings();"));
        assert!(EMBEDDED_INDEX.contains("const settings = selectedSimulationSettings();"));
        assert!(EMBEDDED_INDEX.contains("seed: settings.seed ?? Math.floor(Math.random() * 1e9)"));
        assert!(EMBEDDED_INDEX.contains("const changed = human || (activeSimulationSettingsKey"));
        assert!(EMBEDDED_INDEX.contains("spectate: gameMode === \"ai_sim\","));
        assert!(
            !EMBEDDED_INDEX.contains("num_city_states: cityStates"),
            "the lobby should not send a custom city-state count"
        );
        assert!(
            !EMBEDDED_INDEX.contains("max_turns: turnLimit"),
            "the turn cap should come from the selected game speed"
        );
        // None of the machinery that used to stand in for an unmade decision
        // survives anywhere in the client.
        for gone in [
            "?????",
            "RANDOM_SETTING",
            "randomSettings",
            "feelingLucky",
            "luckybtn",
            "randomnote",
            "civvis-random-settings-v1",
            "victory-random",
            "indeterminate",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(gone),
                "the client still carries the settings-left-to-chance machinery: {gone}"
            );
        }
    }

    /// Teams are chosen in the lobby and are permanent once the world exists,
    /// so the division the browser asked for has to be readable back out of
    /// `/state`: a page that reloads over a team game offers to restart *that*
    /// game rather than a free-for-all with the same map.
    #[test]
    fn a_team_division_reaches_the_world_and_comes_back_in_the_state() {
        let stock = current();
        let small = json!({"num_players": 4, "width": 20, "height": 14, "num_city_states": 1});
        let mut request = small.clone();
        request["teams"] = json!([0, 0, 1, 1]);
        let params = new_game_params(&stock, &request);
        assert_eq!(params.teams, vec![Some(0), Some(0), Some(1), Some(1)]);
        let session = Session::new(params);
        assert_eq!(session.state()["teams"], json!([0, 0, 1, 1]));
        // A free-for-all is every seat playing for itself, said out loud, so
        // the lobby can tell it from a world that never mentioned teams.
        let alone = Session::new(new_game_params(&stock, &small));
        assert_eq!(alone.state()["teams"], json!([null, null, null, null]));
        // The staged world carries the same division, or the exhibition's next
        // game would quietly drop it between staging and starting.
        let staged = shared_for(Session::new(current()));
        staged.stage_next_game_settings(&request);
        assert_eq!(
            decorated_state(&staged)["next_game_settings"]["teams"],
            json!([0, 0, 1, 1])
        );
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
        // Fixed geography changes the coastline source, not the selected
        // shape: Earth can be sampled onto a flat atlas too.
        let earth = new_game_params(
            &flat,
            &json!({"map_script": "true_start_earth", "map_topology": "flat"}),
        );
        assert_eq!(earth.map_topology, MapTopology::Flat);
        assert_eq!((earth.width, earth.height), (size.width, size.height));
    }

    #[test]
    fn globe_tilt_rotates_the_camera_without_squashing_its_limb() {
        // A flat chart is allowed to use a ground-plane projection.  Once the
        // world is known to be round, pitch must instead be part of the 3D view
        // basis, so the orthographic ocean, rim, tiles, and hit cells retain a
        // circular silhouette at every tilt.
        assert!(EMBEDDED_INDEX.contains("function planetCameraBase()"));
        assert!(EMBEDDED_INDEX.contains("function planetTiltBasis(basis, tilt = cam.tilt)"));
        assert!(EMBEDDED_INDEX.contains("planetSpin(basis, basis.right, angle)"));
        assert!(EMBEDDED_INDEX.contains("function planetUntiltBasis(basis, tilt = cam.tilt)"));
        assert!(EMBEDDED_INDEX.contains(
            "...planetBasisCamera(planetTiltBasis(planetViewBasis(camera)))"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const view = planetBasisCamera(planetUntiltBasis(basis));"
        ));
        assert!(EMBEDDED_INDEX.contains("function planetGroundProjection()"));
        assert!(EMBEDDED_INDEX.contains("return knowsGlobe() ? 1 : cameraTiltProjection();"));
        assert!(EMBEDDED_INDEX.contains("const projection = planetGroundProjection();"));
    }

    #[test]
    fn browser_orders_controls_interface_setup_and_logs() {
        // Readability is a shared interface contract, not a collection of
        // one-off enlargements. Panels inherit one system stack and a named
        // scale with a 9px floor; map labels use the same platform-native
        // stack instead of depending on an unbundled webfont.
        assert!(EMBEDDED_INDEX.contains("--font-ui: system-ui"));
        assert!(EMBEDDED_INDEX.contains("--type-micro: 9px;"));
        assert!(EMBEDDED_INDEX.contains("--type-body: 14px;"));
        assert!(EMBEDDED_INDEX.contains("font: var(--type-body)/1.5 var(--font-ui);"));
        assert!(EMBEDDED_INDEX.contains("text-size-adjust: 100%"));
        assert!(EMBEDDED_INDEX.contains(
            "9px system-ui, -apple-system, BlinkMacSystemFont, sans-serif"
        ));
        for illegible in [
            "font-size: 5.5px",
            "font-size: 6px",
            "font-size: 7px",
            "font-size: 8px",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(illegible),
                "browser CSS should not restore the illegible {illegible} declaration"
            );
        }
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
        // The game mode leads the primary path. The advanced ruleset and the
        // start-era ladder still come from the server, so a new ruleset — or a
        // rung somebody finally builds — never means editing the markup.
        assert!(EMBEDDED_INDEX.contains("id=\"baseruleset\""));
        assert!(EMBEDDED_INDEX.contains(">Base game ruleset<"));
        assert!(EMBEDDED_INDEX.contains("RULES.base_rulesets.map(ruleset =>"));
        assert!(EMBEDDED_INDEX.contains("id=\"startera\""));
        assert!(EMBEDDED_INDEX.contains(">Start era<"));
        assert!(EMBEDDED_INDEX.contains("RULES.start_eras.map(era =>"));
        assert!(EMBEDDED_INDEX.contains("base_ruleset: baseRuleset, start_era: startEra,"));
        assert!(
            EMBEDDED_INDEX.find(">Game mode<") < EMBEDDED_INDEX.find(">Base game ruleset<"),
            "the game mode must lead the primary setup path"
        );
        // The eon that used to sit above the era is gone from the lobby, and
        // the ladder it hung off with it.
        assert!(!EMBEDDED_INDEX.contains("id=\"starteon\""));
        assert!(!EMBEDDED_INDEX.contains(">Start eon<"));
        assert!(!EMBEDDED_INDEX.contains("start_eon"));
        assert!(EMBEDDED_INDEX.contains("id=\"leaderpool\""));
        assert!(EMBEDDED_INDEX.contains(">Civ 6 Leaders</option>"));
        assert!(EMBEDDED_INDEX.contains(">Expanded Historical Figures</option>"));
        assert!(EMBEDDED_INDEX.contains(">Today's Leaders — roster pending</option>"));
        assert!(
            EMBEDDED_INDEX.find(">Civ 6 Leaders</option>")
                < EMBEDDED_INDEX.find(">Expanded Historical Figures</option>")
                && EMBEDDED_INDEX.find(">Expanded Historical Figures</option>")
                    < EMBEDDED_INDEX.find(">Today's Leaders — roster pending</option>"),
            "leader tiers must remain Civ 6, historical, then today"
        );
        assert!(EMBEDDED_INDEX.contains("leader_pool: leaderPool"));
        assert!(EMBEDDED_INDEX.contains("function syncLeaderPool()"));
        assert!(EMBEDDED_INDEX.contains("RULES.leader_pools"));
        assert!(EMBEDDED_INDEX.contains("True Start:"));
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
        assert!(EMBEDDED_INDEX.contains("resetMapFacing()"));
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
        assert!(EMBEDDED_INDEX.contains("function planetGroundDrag(dx, dy)"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(dragState.basis, turnX, turnY))"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(touchGesture.basis, turnX, turnY))"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(basis, dx, dy))"));
        assert!(EMBEDDED_INDEX.contains("spin:planetGlide(released.vpx, released.vpy)"));
        // Zooming shares that turn too, and it aims at a world rather than at a
        // pixel. Out in the system a body is a few pixels across — the Moon is
        // four on the whole-system shot — so an anchor held to the raw point of
        // space under the pointer demanded an aim nobody can manage and walked
        // off into empty sky when it was missed: measured at twelve pixels wide
        // of the Moon, sixteen wheel steps finished three thousand pixels away
        // with nothing at all on the stage. So every world claims a halo, the
        // strongest claim takes the pointer, and a pointer on nothing is
        // therefore taken by the roughly nearest world. What the pointer's aim
        // is *for* changes with how big that world is drawn: travel while it is
        // a marble, and once it is the place underfoot the world turns until the
        // ground that was under the pointer is back under it, which is the same
        // lean a flat map has always had and which the globe recovered three per
        // cent of before this. The ceiling comes from the world being flown to,
        // not from whichever marble happens to be nearest the frame's middle.
        assert!(EMBEDDED_INDEX.contains("function skyPointerWorld(sx, sy, radius = planetEarthRadius(), pan = SKY_PAN)"));
        assert!(EMBEDDED_INDEX.contains("function skyWorldGrab(drawn)"));
        assert!(EMBEDDED_INDEX.contains("function skyZoomAim(sx, sy, radius = planetEarthRadius(), pan = SKY_PAN)"));
        assert!(EMBEDDED_INDEX.contains("function skySurfacePoint(body, sx, sy, radius, pan = SKY_PAN)"));
        assert!(EMBEDDED_INDEX.contains("function skyLean(lean, ease)"));
        assert!(EMBEDDED_INDEX.contains(
            "applyPlanetBasis(planetSpin(basis, axis.map(value => value / length), -owed * ease));",
        ));
        assert!(EMBEDDED_INDEX.contains("if (!body || !lean.point || body.id !== \"earth\") return 0;"));
        assert!(EMBEDDED_INDEX
            .contains("function planetMaxScale(pan = SKY_PAN, body = skyNearestWorld(pan))"));
        assert!(EMBEDDED_INDEX.contains("const ceiling = planetMaxScale(basePan, subject);"));
        assert!(EMBEDDED_INDEX.contains("const aim = skyAnchor"));
        assert!(EMBEDDED_INDEX.contains("cameraZoom = {kind:\"planet\", scale, pan, lean};"));
        assert!(EMBEDDED_INDEX
            .contains("const leanLeft = cameraZoom.lean ? skyLean(cameraZoom.lean, ease) : 0;"));
        // The old raw-point anchor, and the early return that left a chart with
        // no lean at all, must both be gone: a chart has no system to travel
        // through, but it leans towards the pointer exactly as a flat map does.
        assert!(!EMBEDDED_INDEX.contains("const pointerX = skyAnchor?.x ??"));
        assert!(!EMBEDDED_INDEX.contains("const scale = planetScaleClampAt(base * f, {x:0, y:0});"));
        // A notch of zoom is a fraction of the ladder in front of it, not a
        // fixed ratio: with a fixed ratio the far stop is a hundred and sixty
        // wheel notches from the ground and the far half of the sky can be
        // built and never reached. Every zoom that arrives as a step of intent goes
        // through the gearing; a pinch is absolute and is geared at its own
        // site, against the spread the fingers started from.
        assert!(EMBEDDED_INDEX.contains("function skyZoomPace(scale = cam.scale, pan = SKY_PAN)"));
        assert!(EMBEDDED_INDEX.contains("function skyZoomStep(f)"));
        assert!(EMBEDDED_INDEX
            .contains("const pace = skyZoomLadder() / (SKY_ZOOM_SWEEPS * FLAT_ZOOM_LADDER);"));
        assert!(EMBEDDED_INDEX.contains("zoomAt(skyZoomStep(factor), ev.clientX - r.left, ev.clientY - r.top);"));
        assert!(EMBEDDED_INDEX.contains("zoomAt(skyZoomStep(1.35))"));
        assert!(EMBEDDED_INDEX.contains("zoomAt(skyZoomStep(1 / 1.35))"));
        assert!(EMBEDDED_INDEX
            .contains("const want = touchGesture.scale * Math.pow(spread, touchGesture.pace || 1);"));
        // And the divisor under it has to sit below every scale that can really
        // be standing there. At `1e-4` it sat above most of the sky, so past
        // Jupiter a pinch opening the fingers slammed the camera to the far
        // stop, the wrong way, in one move — everything out there was
        // unreachable by touch.
        assert!(EMBEDDED_INDEX
            .contains("zoomAt(want / Math.max(1e-30, base), mx, my, touchGesture.skyAnchor);"));
        assert!(!EMBEDDED_INDEX.contains("want / Math.max(1e-4, base)"));
        // A flat board never sees any of it, and a people who have not proved
        // their world round have no ladder to gear: `skyZoomStep` hands the
        // factor straight back, and the pace floors at one.
        assert!(EMBEDDED_INDEX.contains("  if (!planetMap()) return f;"));
        // The arrival, and the two halves of it that have to hold together. A
        // world's drawn size is a property of the zoom alone, so the camera has
        // to be *at* the body as well — otherwise a tile's zoom over the
        // Atlantic reads as an arrival at the Moon, which is nominally four
        // stages wide there. How slow an arrival is nobody chooses: the gearing
        // is handed back until one notch is the notch the flat board has.
        assert!(EMBEDDED_INDEX.contains("function skyArrival(body, radius, pan)"));
        assert!(EMBEDDED_INDEX.contains(
            "  return size * Math.max(0, 1 - away / (span * .6 + drawn));",
        ));
        assert!(EMBEDDED_INDEX
            .contains("return pace / (1 + (pace - 1) * Math.max(0, Math.min(1, arrival)));"));
        // Home is one of the arrivals, so standing on the board the gearing is
        // fully off and a zoom over the map is the zoom it has always been.
        // Every notch this lengthens is a notch out in the dark.
        assert!(EMBEDDED_INDEX.contains("const SKY_ARRIVALS = [\"earth\", \"moon\", \"mars\", \"exo\"];"));
        // And the destination's own star with them, because out there the stop
        // belongs to the star: LHS 1140 is twelve times its own planet and sits
        // at the same catalogue point, so `planetMaxScale` answers with the
        // star's ceiling and the zoom ends while the planet is still a bead.
        // Keyed to the planet alone the arrival never got past 0.46.
        assert!(EMBEDDED_INDEX.contains("const star = skyTarget(st)?.star;"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"planet\" selected>Planet</option>"));
        let map_menu = EMBEDDED_INDEX
            .split("id=\"maptype\"")
            .nth(1)
            .and_then(|tail| tail.split("</select>").next())
            .expect("browser map selector");
        for option in [
            "<option value=\"earth\">Earth</option>",
            "<option value=\"true_start_earth\">True Start Earth</option>",
            "<option value=\"fjords\">Fjords</option>",
        ] {
            assert!(map_menu.contains(option), "missing {option}");
        }
        let option_at = |value: &str| map_menu.find(value).expect("map option");
        assert!(
            option_at("value=\"earth\"")
                < option_at("value=\"true_start_earth\"")
                && option_at("value=\"true_start_earth\"") < option_at("value=\"continents\"")
                && option_at("value=\"small_continents\"") < option_at("value=\"fjords\""),
            "Earth, True Start Earth, Continents, Small Continents, and Fjords must keep their requested order"
        );
        // The world's shape and its poles are settings of their own, and the
        // renderer picks its projection from the shape the world reports
        // rather than from the world type it was filled with.
        assert!(EMBEDDED_INDEX.contains("id=\"mapshape\""));
        assert!(EMBEDDED_INDEX.contains("id=\"mappoles\""));
        assert!(EMBEDDED_INDEX.contains("return state.map.shape === \"planet\""));
        assert!(EMBEDDED_INDEX.contains("<option value=\"land_only\">Land Only</option>"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"water_world\">Water World</option>"));
        assert!(EMBEDDED_INDEX.contains("RULES.map_topologies"));
        assert!(EMBEDDED_INDEX.contains("const chosen = select.value;"));
        assert!(EMBEDDED_INDEX.contains(
            "const defaultValue = id === \"mapshape\" && list.some(entry => entry.id === \"planet\")"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if ([...select.options].some(option => option.value === chosen)) select.value = chosen;"
        ));
        assert!(EMBEDDED_INDEX.contains("st.map.shape || \"planet\""));
        assert!(EMBEDDED_INDEX.contains("shape.disabled = false"));
        assert!(!EMBEDDED_INDEX.contains("if (earth) shape.value = \"planet\""));
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
        assert!(EMBEDDED_INDEX
            .contains("async function startNewSimulation(restartSource = \"manual\")"));
        assert!(EMBEDDED_INDEX
            .contains("const payload = {...newSimulationPayload(), paused: wasPaused}"));
        assert!(EMBEDDED_INDEX.contains("setPace({paused: wasPaused})"));
        assert!(EMBEDDED_INDEX.contains(
            "sessionStorage.setItem(\"civvis-restart-paused-v1\", handoff.paused ? \"1\" : \"0\")"
        ));
        assert!(EMBEDDED_INDEX.contains("<html>"));
        assert!(!EMBEDDED_INDEX.contains("<html class=\"world-loading\">"));
        assert!(EMBEDDED_INDEX.contains("<body aria-busy=\"false\">"));
        assert!(!EMBEDDED_INDEX.contains("Joining the world"));
        assert!(EMBEDDED_INDEX.contains("id=\"world-transition\""));
        assert!(EMBEDDED_INDEX.contains(
            "document.documentElement.classList.add(\"world-loading\", \"world-restarting\")"
        ));
        assert!(EMBEDDED_INDEX.contains("sessionStorage.setItem(\"civvis-world-transition-v1\""));
        // Losing the socket is what a process handoff *is*, so the veil does
        // not call the ordinary case a reconnection.
        assert!(EMBEDDED_INDEX.contains("const HANDOFF_QUIET_MS = 2500;"));
        assert!(EMBEDDED_INDEX.contains(
            "setWorldTransitionStage(Date.now() - watchStartedAt < HANDOFF_QUIET_MS"
        ));
        assert!(EMBEDDED_INDEX.contains("await settingsStageChain.catch(() => {})"));
        // A result timer belongs to one exact world. Background-tab timer
        // throttling can let it wake after the supervisor has already put a
        // successor on the same port, so both the firing edge and the server
        // request carry that original process/seed identity.
        assert!(EMBEDDED_INDEX.contains("let finaleCountdownWorld = null;"));
        assert!(EMBEDDED_INDEX.contains("if (finaleCountdownTimer === null) return;"));
        assert!(EMBEDDED_INDEX.contains(
            "state.server_instance !== finaleCountdownWorld.serverInstance"
        ));
        assert!(EMBEDDED_INDEX.contains("state.seed !== finaleCountdownWorld.seed"));
        assert!(EMBEDDED_INDEX.contains("startNewSimulation(\"finale_countdown\")"));
        assert!(EMBEDDED_INDEX.contains("const finishedInstance = handoff.finishedInstance;"));
        assert!(EMBEDDED_INDEX.contains("const finishedSeed = handoff.finishedSeed;"));
        assert!(EMBEDDED_INDEX.contains("replace_world: {"));
        assert!(EMBEDDED_INDEX.contains("restart_source: restartSource"));
        assert!(EMBEDDED_INDEX.contains(
            "server_instance: finishedInstance, seed: finishedSeed"
        ));
        assert!(EMBEDDED_INDEX.contains("specFetching || specPending || worldTransitionPending()"));
        assert!(EMBEDDED_INDEX.contains("specFetchAbort?.abort()"));
        assert!(EMBEDDED_INDEX.contains("worldTransitionHandoff.supervised"));
        assert!(EMBEDDED_INDEX
            .contains("String(st?.seed) === String(worldTransitionHandoff.targetSeed)"));
        assert!(EMBEDDED_INDEX.contains("finishWorldTransition(st);"));
        assert!(EMBEDDED_INDEX.contains("clearWorldTransition();"));
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
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/runtime\", {cache: \"no-store\"}, 500)"));
        assert!(EMBEDDED_INDEX.contains("render(adoptTiles(first), true, true);"));
        assert!(EMBEDDED_INDEX.contains("st.seed !== state.seed"));
        assert!(!EMBEDDED_INDEX.contains("id=\"head-newgame\""));
        // The mode still decides whether anyone is watching or playing.
        assert!(EMBEDDED_INDEX.contains("spectate: gameMode === \"ai_sim\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"specchk\""));
        assert!(!EMBEDDED_INDEX.contains("RULES.map_sizes.filter"));

        // The lobby's reading order. `#newgame-options` is a two-column grid
        // filled row by row, so document order is the visible reading path:
        // mode and size, then geography, then era and clock sharing one row,
        // then victories. The rules, roster, teams, climate and seed stay in
        // the advanced drawer; the era mods live in the mods drawer.
        let order = [
            "gamemode",
            "np",
            "mapshape",
            "maptype",
            "startera",
            "gamespeed",
        ]
        .map(|setting| {
            EMBEDDED_INDEX
                .find(&format!("id=\"{setting}\""))
                .unwrap_or_else(|| panic!("browser setup is missing the {setting} select"))
        });
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "lobby order must read mode/size, shape/map, era/speed"
        );
        assert!(EMBEDDED_INDEX.find("id=\"gamespeed\"").unwrap()
            < EMBEDDED_INDEX.find("id=\"victory-options\"").unwrap());
        assert!(EMBEDDED_INDEX.find("id=\"game-mod-settings\"").unwrap()
            < EMBEDDED_INDEX.find("id=\"futureera\"").unwrap());
        // The advanced drawer starts with the ruleset, team split and leader
        // roster. The custom table sits under its selection mode, followed by
        // climate and seed. Human-only leader/difficulty fields remain there
        // too, so they do not interrupt the primary simulation path.
        for advanced in [
            "class=\"small game-advanced-setting\" data-advanced-order=\"10\">Base game ruleset",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"20\">Teams",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"30\">Leader pool",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"40\">Leader selection",
            "class=\"custom-leader-selection game-advanced-setting civ6-hidden\" data-advanced-order=\"45\"",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"50\">Thermal distribution",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"60\">Map seed",
        ] {
            assert!(EMBEDDED_INDEX.contains(advanced), "missing advanced setting: {advanced}");
        }
        for normal in [
            "class=\"small civ6-hidden\">World shape",
            "class=\"small civ6-hidden\">Start era",
            "class=\"small era-future-setting\">Future era",
            "class=\"victory-options civ6-hidden\" id=\"victory-options\"",
        ] {
            assert!(EMBEDDED_INDEX.contains(normal), "missing normal setting: {normal}");
        }
        assert!(EMBEDDED_INDEX.contains(
            "[\"gamemode\", true], [\"np\", true], [\"mapshape\", true], [\"maptype\", true],"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "[\"startera\", true], [\"gamespeed\", true],"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "basic.push(document.getElementById(\"victory-options\"), document.getElementById(\"saves-group\"));"
        ));
        // Display Settings is a short reading path: observer controls first,
        // then the overlay menus — the panels a viewer reaches for most — then
        // map options, and finally performance.
        let display_sections = [
            "<div class=\"observer-settings\" id=\"observer-settings\">",
            "<details class=\"display-settings-group\" id=\"map-overlays-settings\"",
            "<details class=\"display-settings-group\" id=\"map-options-settings\"",
            "<details class=\"display-settings-group\" id=\"performance-options-settings\"",
        ];
        assert!(
            display_sections.windows(2).all(|pair| {
                EMBEDDED_INDEX.find(pair[0]).unwrap() < EMBEDDED_INDEX.find(pair[1]).unwrap()
            }),
            "Display Settings sections should read observer, overlays, map, performance"
        );
        // Performance options read outward from the one control the viewer can
        // change: the resolution being drawn at, the map that resolution has to
        // cover, how fast this browser draws it, and last the simulation behind
        // it — turn cost, a whole run's projected length, the run on screen, and
        // the runs this page has already watched end to end.
        let performance_markers = [
            "id=\"renderresolution\"",
            "id=\"performance-canvas-value\"",
            "id=\"performance-workload-value\"",
            "id=\"performance-render-value\"",
            "id=\"performance-slow-value\"",
            "id=\"performance-simulation-value\"",
            "id=\"performance-fullsim-value\"",
            "id=\"performance-thissim-value\"",
            "id=\"performance-lastsim-value\"",
            "id=\"performance-simcount-value\"",
            "id=\"performance-recommendation\"",
        ];
        for performance_marker in performance_markers {
            assert!(
                EMBEDDED_INDEX.contains(performance_marker),
                "performance display is missing: {performance_marker}"
            );
        }
        assert!(
            performance_markers.windows(2).all(|pair| {
                EMBEDDED_INDEX.find(pair[0]).unwrap() < EMBEDDED_INDEX.find(pair[1]).unwrap()
            }),
            "performance options should read resolution, canvas, workload, render, slow \
             frames, then the simulation rows"
        );
        // The simulation rows are the reason the panel can answer "how long does
        // a whole game take" at all, and every build must be able to fill them:
        // the published browser build reports no turn cost of its own, so the
        // viewer measures the turn interval it was served instead. That
        // substitute is only honest while the game plays itself — between a
        // human's turns the same interval is how long that person thought — so
        // both the fallback and the ledger are gated on spectating.
        for simulation_contract in [
            "function simulationTurnMs() {",
            "if (viewerPerformance.simulation.wall !== null) return viewerPerformance.simulation.wall;",
            "return viewerPerformance.spectator ? simulationLedger.observedTurnMs : null;",
            "const SIMULATION_FALLBACK_TURN_LIMIT = 250;",
            "function trackSimulationRun(st, now) {",
            "if (st?.spectate !== true) return;",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(simulation_contract),
                "simulation statistics contract is missing: {simulation_contract}"
            );
        }
        // Resource names are the default detailed-map treatment, but the
        // existing compact symbol stays an explicit, persisted choice and the
        // only treatment at survey scale.
        for resource_display in [
            "id=\"resourcedisplay\" aria-label=\"Resource display\"",
            "<option value=\"symbol_word\" selected>Symbol &amp; word</option>",
            "<option value=\"symbol\">Symbol</option>",
            ".resource-display-setting { grid-template-columns: minmax(0, 1fr); gap: 4px; }",
            ".resource-display-setting > select { width: 100%; }",
            ".display-settings-group-body > .speed-row,",
            "const RESOURCE_DISPLAY_STORAGE_KEY = \"civvis-resource-display\";",
            "let RESOURCE_DISPLAY = localStorage.getItem(RESOURCE_DISPLAY_STORAGE_KEY) === \"symbol\"",
            "function setResourceDisplay(mode) {",
            "resourceDisplay.onchange = () => setResourceDisplay(resourceDisplay.value);",
            "function drawResourceWordBadge(t, x, y, rim = resourceBadgeRim(t)) {",
            "drawResourcePictogram(t.resource, rx, iconY - .1, RES_WORD_ICON_SIZE);",
            "cx.strokeText(label, rx, textY, RES_WORD_MAX_WIDTH);",
            "if (!resourceIsImproved(t)) return \"#fffdf3\";",
            "? jerseyLanes(owner)[1] : rim;",
            "function drawResourcePillageStrike(x, y, halfWidth, ink) {",
            "if (RESOURCE_DISPLAY === \"symbol_word\" && cam.scale >= RES_WORD_LABEL_SCALE) {",
            "drawResourceSymbolBadge(t, x, y, rim);",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(resource_display),
                "resource display contract is missing: {resource_display}"
            );
        }
        assert!(EMBEDDED_INDEX.contains(
            "let SHOW_ROCKET_ANIMATIONS = localStorage.getItem(\"civvis-show-rocket-animations\") !== \"0\";"
        ));
        // The map's painter order is part of its interaction contract: roads
        // are ground infrastructure, resource marks sit below command tokens,
        // and the optional yield overlay is the reader's top-most map layer.
        // Keep both projections in lockstep so switching to the globe does not
        // invert the information hierarchy.
        let flat_map = EMBEDDED_INDEX.find("function drawScene() {").unwrap();
        let flat = &EMBEDDED_INDEX[flat_map..];
        let flat_roads = flat.find("drawFlatStrategicRoads(tiles);").unwrap();
        let flat_borders = flat.find("// --- territory borders:").unwrap();
        let flat_resources = flat
            .find("if (resourceMarkerVisible(t)) drawResourceBadge(t, x, y);")
            .unwrap();
        let flat_units = flat.find("// --- units").unwrap();
        let flat_yields = flat
            .find("drawFlatTileYieldOverlay(tiles, workedSet, visSet);")
            .unwrap();
        assert!(
            flat_roads < flat_borders
                && flat_borders < flat_resources
                && flat_resources < flat_units
                && flat_units < flat_yields,
            "flat-map painter order must be roads, borders/resources, units, then yields"
        );
        let globe_map = EMBEDDED_INDEX.find("function drawPlanetMap() {").unwrap();
        let globe = &EMBEDDED_INDEX[globe_map..];
        let globe_roads = globe
            .find("drawPlanetStrategicRoads(cells, turnVisible, spectator);")
            .unwrap();
        let globe_borders = globe
            .find("if (radius >= SKY_MARKERS) drawPlanetFrontiers(cells);")
            .unwrap();
        let globe_terrain = globe
            .find("drawPlanetStrategicTerrain(cells, turnVisible, spectator);")
            .unwrap();
        let globe_units = globe.find("const visibleUnits").unwrap();
        let globe_yields = globe
            .find("drawPlanetTileYieldOverlay(cells, turnVisible, spectator);")
            .unwrap();
        assert!(
            globe_roads < globe_borders
                && globe_borders < globe_terrain
                && globe_terrain < globe_units
                && globe_units < globe_yields,
            "globe painter order must match the flat map's road/resource/unit/yield hierarchy"
        );
        assert!(EMBEDDED_INDEX.contains("const LUMBER_MILL_ICON_SCALE = 1.3;"));
        for satellite_animation in [
            "function drawSkySatellites(crew, camera, radius, centerX, centerY, alpha, now) {\n  if (!SHOW_ROCKET_ANIMATIONS) return;",
            "function drawFlatSatellites(now) {\n  if (!SHOW_ROCKET_ANIMATIONS) return;",
            "return (SHOW_ROCKET_ANIMATIONS && crews.satellite.length > 0) || crews.expedition.length > 0;",
            "return SHOW_ROCKET_ANIMATIONS && (activeSkyLaunches().length > 0 ||",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(satellite_animation),
                "rocket animation setting must also govern satellites: {satellite_animation}"
            );
        }
        // Teams are a division of the table, so they sit beside the setting
        // that says who is at it. A world opens free-for-all, and the splits a
        // size can seat are named in the option rather than left to be found
        // by trying one.
        assert!(EMBEDDED_INDEX.contains("Teams<select id=\"teams\""));
        assert!(EMBEDDED_INDEX.contains("<option value=\"ffa\" selected>Free-for-all</option>"));
        assert!(EMBEDDED_INDEX.contains("const TEAM_RULES = [\"2\", \"3\", \"4\", \"pairs\"];"));
        assert!(EMBEDDED_INDEX.contains("option.disabled = !split;"));
        // The world size decides which splits exist, so it re-fits them before
        // the panel's own delegated listener stages what is now selected.
        assert!(EMBEDDED_INDEX.contains(
            "document.getElementById(\"np\").addEventListener(\"change\", () => {\n  syncTeams();\n  syncCustomLeaderSelection();\n});"
        ));
        // The server is handed the seat-by-seat assignment, never the rule
        // that produced it; a world on screen is read back the other way.
        assert!(EMBEDDED_INDEX.contains("teamAssignment(np, readSetting(\"teams\"))"));
        assert!(EMBEDDED_INDEX.contains("leader_selection: leaderSelection"));
        assert!(EMBEDDED_INDEX.contains("id=\"custom-leader-selection\""));
        assert!(EMBEDDED_INDEX.contains("data-custom-team"));
        assert!(EMBEDDED_INDEX.contains("data-custom-civ"));
        assert!(EMBEDDED_INDEX.contains("data-custom-elo"));
        assert!(EMBEDDED_INDEX.contains("RULES?.leader_elo_options"));
        assert!(EMBEDDED_INDEX.contains("id=\"game-mod-settings\""));
        assert!(EMBEDDED_INDEX.contains("id=\"game-mod-summary\""));
        assert!(EMBEDDED_INDEX.contains("teamRuleFromArray(st.teams)"));
        assert!(EMBEDDED_INDEX.contains("teamRuleFromArray(settings.teams)"));
        // Heat is a setting about climate, not about whether two ice caps
        // exist, so it is named for what it decides.
        assert!(EMBEDDED_INDEX.contains("Thermal distribution<select id=\"mappoles\""));
        assert!(EMBEDDED_INDEX.contains("<option value=\"randomized\">Randomized</option>"));
        assert!(!EMBEDDED_INDEX.contains("Poles<select id=\"mappoles\""));
        // And it offers two worlds, not three: heat either follows latitude or
        // it doesn't. The world with no cold end at all is retired, so it is
        // gone from the markup as well as from `MAP_POLES` — the select is
        // rebuilt from that list on load, and the two have to say the same
        // thing or the lobby offers a world the engine will not build.
        let thermal_setting = EMBEDDED_INDEX.find("id=\"mappoles\"").unwrap();
        let thermal_options = {
            let tail = &EMBEDDED_INDEX[thermal_setting..];
            &tail[..tail.find("</select>").expect("unterminated thermal select")]
        };
        assert_eq!(
            thermal_options.matches("<option").count(),
            2,
            "thermal distribution offers exactly two worlds"
        );
        assert!(!thermal_options.contains("no_poles"));
        assert_eq!(MAP_POLES.len(), 2);
        for spec in MAP_POLES {
            assert!(
                thermal_options.contains(&format!("value=\"{}\"", spec.id)),
                "the lobby is missing {}",
                spec.id
            );
        }

        let title = EMBEDDED_INDEX
            .find("<div class=\"side-head\">")
            .expect("sidebar title");
        let sidebar = EMBEDDED_INDEX
            .find("<div class=\"side-scroll\">")
            .expect("scrolling sidebar");
        let game_settings = EMBEDDED_INDEX
            .find("<details class=\"sidebar-section\" id=\"setupsec\"")
            .expect("game settings panel");
        let simulation_controls = EMBEDDED_INDEX
            .find("<div class=\"side-actions\" aria-label=\"Simulation controls\">")
            .expect("simulation controls");
        let interface_settings = EMBEDDED_INDEX
            .find("<details class=\"sidebar-section\" data-section=\"display-settings\">")
            .expect("display settings panel");
        let event_log = EMBEDDED_INDEX
            .find("<span>Game event log</span>")
            .expect("game event log");
        let war_log = EMBEDDED_INDEX
            .find("<span>War log</span>")
            .expect("war log");
        let strategy = EMBEDDED_INDEX
            .find("<span>AI strategy</span>")
            .expect("AI strategy section");
        let government = EMBEDDED_INDEX
            .find("data-section=\"government\"")
            .expect("government section");
        let keyboard_shortcuts = EMBEDDED_INDEX
            .find("<summary>Keyboard shortcuts</summary>")
            .expect("keyboard shortcuts");
        let map_lenses = EMBEDDED_INDEX
            .find("<details id=\"maplensessec\" data-section=\"map-lenses\">")
            .expect("collapsible map lens section");
        let map_utility = EMBEDDED_INDEX
            .find("<div id=\"map-utility-panel\"")
            .expect("map utility band");
        let visible_tile_search = EMBEDDED_INDEX
            .find("<div id=\"map-search\"")
            .expect("visible tile search");
        // The controls must be ready before the settings they act on: first
        // the buttons, then the interface controls, then the game setup.
        assert!(title < sidebar && sidebar < simulation_controls);
        assert!(simulation_controls < interface_settings && interface_settings < game_settings);
        assert!(game_settings < strategy);
        // The column runs deepest-cause first — a civilization's active plan
        // and its decision factors, then wars, then the world's record of what
        // happened — so reading the column downward keeps each answer beside
        // the reasons that formed it.
        assert!(
            strategy < war_log
                && war_log < event_log
                && event_log < government
                && government < map_lenses
                && map_lenses < map_utility
                && map_lenses < visible_tile_search
                && visible_tile_search < keyboard_shortcuts,
            "left panel should show controls, display settings, game setup, the AI strategy dossier, \
             world logs, government, map lenses, visible-tile search and then keyboard shortcuts"
        );
        assert!(EMBEDDED_INDEX.contains("<span>Display Settings</span>"));
        assert!(!EMBEDDED_INDEX.contains("<span>Interface Settings</span>"));
        assert!(EMBEDDED_INDEX.contains(
            "order: -1; width: clamp(220px, 18vw, 332px); min-width: clamp(220px, 18vw, 332px);"
        ));
        for overlay in ["players", "victory", "minimap", "controls", "lenses"] {
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
        // Each overlay owns a menu with its visibility switch in the header,
        // and the order the menus are written in follows the map: the rail
        // read top to bottom — standings, victory tracker, world minimap —
        // and then the map controls and lenses docked together in the
        // opposite corner.
        assert!(EMBEDDED_INDEX
            .contains(".overlay-option-grid { display: grid; grid-template-columns: 1fr 1fr;"));
        let switches = EMBEDDED_INDEX
            .split_once(
                "<div class=\"overlay-options overlay-menus\" role=\"group\" aria-labelledby=\"overlay-options-title\">",
            )
            .expect("map overlay switches")
            .1
            .split_once("id=\"reset-overlay-layout\"")
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
            ["players", "victory", "minimap", "controls", "lenses"],
            "the switches read down the rail, then the map dock in the far corner"
        );
        assert!(EMBEDDED_INDEX.contains("id=\"map-lens-exit\""));
        assert!(EMBEDDED_INDEX.contains("body.overlay-lenses-hidden #maplensessec"));
        // Lenses are a docked panel at the deck's lower edge: collapsed by
        // default, saved with the other disclosures, and always at the bottom
        // of the screen in reading order — lenses, then visible-tile search,
        // then keyboard shortcuts — so changing the map perspective never
        // requires scrolling the command sections.
        let lens_section = &EMBEDDED_INDEX[map_lenses..map_utility];
        let lens_opening = lens_section
            .split_once('>')
            .expect("map lens section opening tag")
            .0;
        let lenses = lens_section.find("<div id=\"map-lenses\"").expect("map lens menu");
        let strip = lens_section.find("<div id=\"map-lens-strip\"").expect("lens grid");
        let close = lens_section
            .find("data-overlay-close=\"lenses\"")
            .expect("lens dismiss control");
        assert!(
            lenses < strip && close < strip,
            "the collapsible menu header holds its dismiss control and the lens grid follows it"
        );
        assert!(lens_section.contains("<summary>Map lenses</summary>"));
        assert!(lens_section.contains("data-section=\"map-lenses\""));
        // The open grid caps its own height and scrolls inside, so it shares
        // the deck with the scroller above instead of swallowing the column.
        assert!(EMBEDDED_INDEX.contains("#maplensessec[open] .lens-dock-body"));
        assert!(
            !lens_opening.contains(" open"),
            "a fresh profile should start with the map lens section collapsed"
        );
        let utility = &EMBEDDED_INDEX[map_utility..];
        assert!(utility.contains("<div id=\"map-search\""));
        assert!(
            !utility.contains("<div id=\"map-lenses\""),
            "the fixed utility band should not duplicate the collapsible lens section"
        );
        assert!(EMBEDDED_INDEX.contains(
            "#map-lens-strip {\n    display: grid; grid-template-columns: repeat(2, minmax(0, 1fr));"
        ));
        assert!(utility.contains("id=\"map-search-civ\""));
        assert!(EMBEDDED_INDEX.contains(
            "#side > #map-utility-panel #map-search {\n    position: relative; left: auto; right: auto; bottom: auto;"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "#side > .side-scroll { position: relative; z-index: 4; }"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "#side > #map-utility-panel { z-index: 5; }"
        ));
        assert!(EMBEDDED_INDEX
            .contains("document.getElementById(\"map-lens-exit\").onclick = () => setMapLens(null);"));
        assert!(EMBEDDED_INDEX.contains("close.closest(\"#maplensessec\")"));
        // One instrument, one name. The switch, the title bar it is dragged by
        // and the label that follows it across the map all say "World minimap",
        // so nothing in the interface reads as a second, separate world map —
        // the world map is the thing filling the screen behind all of them.
        for name in [
            "<span>World minimap</span>",
            "class=\"overlay-menu-title\">World minimap<small>",
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
        // The standings grow through every civilization shown in the ribbon,
        // with a two-seat floor that keeps the turn counter visible before
        // enough rows exist to do so on their own.
        assert!(EMBEDDED_INDEX.contains("--player-hud-content-height: 106px;"));
        assert!(EMBEDDED_INDEX.contains(
            "--player-hud-max-height: min(var(--player-hud-content-height), calc(100% - 8px));"
        ));
        assert!(EMBEDDED_INDEX.contains("const PLAYER_HUD_MIN_ROWS = 2;"));
        assert!(EMBEDDED_INDEX.contains("const PLAYER_HUD_ROW_PITCH = PLAYER_HUD_ROW_HEIGHT + PLAYER_HUD_ROW_GAP;"));
        assert!(EMBEDDED_INDEX.contains("function playerHudRowPitch()"));
        assert!(EMBEDDED_INDEX.contains("return Math.max(PLAYER_HUD_MIN_HEIGHT, PLAYER_HUD_CHROME_HEIGHT + rows * playerHudRowPitch());"));
        assert!(EMBEDDED_INDEX.contains("maxHeight:playerHudMaxContentHeight"));
        assert!(EMBEDDED_INDEX.contains("maxHeightRatio:.38, avoidsSidebar:true"));
        assert!(EMBEDDED_INDEX.contains("const requestedHeight = playerHudContentHeight(rows);"));
        assert!(EMBEDDED_INDEX.contains(
            "mapArea.style.setProperty(\"--player-hud-content-height\", `${requestedHeight}px`);"
        ));
        assert!(EMBEDDED_INDEX.contains("if (state && hudLayoutGesture?.name !== \"players\") drawPlayerHud();"));
        assert!(EMBEDDED_INDEX.contains(
            "const playerScroll = hud.querySelector(\".diplomacy-ribbon\")?.scrollTop || 0;"
        ));
        assert!(EMBEDDED_INDEX.contains("playerRibbon.scrollTop = playerScroll;"));
        // The wide-screen default has three exact seams: the player HUD fills
        // from its live clear-left edge through the victory tracker's left
        // edge, the tracker owns ten percent of screen width and reaches the
        // minimap's top, and the lower-right world minimap measures its frame
        // diagonal against the screen diagonal. Custom drag layouts still
        // override these values inline, but an uncustomized viewer always
        // returns here.
        assert!(EMBEDDED_INDEX.contains("--victory-hud-width: clamp(156px, 15vw, 280px);"));
        assert!(EMBEDDED_INDEX.contains(
            "right: var(--panel-edge); top: var(--panel-edge);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "width: var(--world-minimap-width); height: var(--minimap-height);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "bottom: calc(var(--minimap-height) + var(--panel-gap) + var(--panel-edge));"
        ));
        assert!(EMBEDDED_INDEX.contains("width: var(--victory-hud-width); height: auto;"));
        assert!(EMBEDDED_INDEX.contains("const WORLD_MINIMAP_DIAGONAL_SHARE = .17;"));
        assert!(EMBEDDED_INDEX.contains("const WORLD_MINIMAP_REFERENCE = {"));
        assert!(EMBEDDED_INDEX.contains("flat: {width:336, height:168},"));
        assert!(EMBEDDED_INDEX.contains("planet: {width:240, height:240},"));
        assert!(EMBEDDED_INDEX.contains(
            "const referenceDiagonal = Math.hypot(reference.width, reference.height);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const diagonalWidth = WORLD_MINIMAP_DIAGONAL_SHARE * Math.hypot(viewportWidth, viewportHeight)"
        ));
        assert!(EMBEDDED_INDEX.contains("const viewport = window.visualViewport;"));
        assert!(EMBEDDED_INDEX.contains(
            "HUD_WIDGETS?.minimap?.element?.classList.toggle(\"minimap-world-planet\", shape === \"planet\");"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "shape === \"planet\" ? wideMinimapWidth"
        ));
        assert!(!EMBEDDED_INDEX.contains(
            ".minimap-frame.minimap-world-planet { width: 164px; height: 150px; }"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "position: absolute; z-index: 1; right: 10px; bottom: 9px;"
        ));
        assert!(!EMBEDDED_INDEX.contains("--player-hud-width"));
        // Every path keeps its top three plus the player's own civilization
        // when that row is lower in this particular victory race.
        assert!(EMBEDDED_INDEX.contains("const focusId = SPEC ? state.view_player : state.player;"));
        assert!(EMBEDDED_INDEX.contains("focusRank >= DEFAULT_VICTORY_LEADERS ? 1 : 0"));
        assert!(EMBEDDED_INDEX.contains(
            "entry.hidden = index >= capacity && entry.dataset.victoryFocus !== \"true\";"
        ));
        assert!(EMBEDDED_INDEX.contains("data-victory-focus=\"${isFocus}\""));
        assert!(EMBEDDED_INDEX.contains("grid-auto-rows: var(--hud-row-height);"));
        // A masthead row is one line: the lock track, then one flat track per
        // visible column in the viewer's own order. Heading and rows read the
        // same list, so the tracks and the cells cannot disagree.
        assert_eq!(
            EMBEDDED_INDEX
                .matches("grid-template-columns: var(--hud-lock-column, 0px) var(--hud-tracks);")
                .count(),
            2,
            "the heading and every row stand on the same flat track list"
        );
        // The values claim their width first and the identity cells flex, so a
        // narrow masthead ellipsizes a name rather than running two figures
        // together. Every data column is a share rather than a pixel count.
        // The shipped stylesheet carries one flat default list; once booted,
        // syncPlayerHudColumns() rewrites it from the viewer's own column
        // model — order, visibility, shares and content-fitted floors alike —
        // together with the table's own floor, below which the standings
        // scroll sideways inside their panel instead of crushing their cells.
        assert!(EMBEDDED_INDEX.contains(
            "minmax(var(--hud-ident-min), 2.035fr)\n      \
             repeat(12, minmax(var(--hud-stat-min), 1fr));"
        ), "the shipped default tracks end with the twelve value columns");
        assert!(EMBEDDED_INDEX.contains("hud.style.setProperty(\"--hud-tracks\", tracks);"));
        assert!(EMBEDDED_INDEX.contains("hud.style.setProperty(\"--hud-table-min\", tableMin);"));
        assert!(EMBEDDED_INDEX.contains("min-width: var(--hud-table-min, 0px);"));
        assert!(EMBEDDED_INDEX.contains("function visiblePlayerHudColumns()"));
        assert!(EMBEDDED_INDEX.contains(
            "const HUD_COLUMN_LAYOUT_STORAGE_KEY = \"civvis-hud-column-layout-v1\";"
        ), "order, visibility and content-fitted floors persist beside the shares");
        // The floors stay in the stylesheet so the width breakpoints can lower
        // them; the shares belong to the viewer. A breakpoint that rewrote a
        // share would undo a dragged column on the next window resize, so no
        // media rule may set either track list or either enclosing track.
        assert!(EMBEDDED_INDEX.contains(
            "--hud-ident-min: 30px; --hud-ident-num-min: 60px; --hud-ident-odds-min: 76px;"
        ));
        assert_eq!(EMBEDDED_INDEX.matches("--hud-ident-num-min:").count(), 1,
            "the Elo floor is declared once and holds at every width: a full \
             score needs the same room on a laptop as on a wall");
        assert!(EMBEDDED_INDEX.contains("const HUD_ELO_MIN_FLOOR = 60;"));
        assert!(EMBEDDED_INDEX.contains("function syncPlayerHudEloFloor()"));
        assert!(EMBEDDED_INDEX.contains(
            "hud.style.setProperty(\"--hud-ident-num-min\", floor);"
        ), "a rating longer than the signed five-digit floor raises its shared track");
        assert_eq!(EMBEDDED_INDEX.matches("--hud-ident-odds-min:").count(), 1,
            "and so does the two-category odds cell's own floor");
        assert!(EMBEDDED_INDEX.contains(
            "grid-template-columns: minmax(0, 1fr) 11px minmax(0, 1fr);"
        ), "Start and Now flank one fixed-width trend column");
        assert!(EMBEDDED_INDEX.contains(
            "playerHudSortHead(\"win_start\", \"START\", \"Win odds at the start of the game\")"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "playerHudSortHead(\"win_delta\", \"Δ\", \"Change in win odds\", \"odds-trend-head\")"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "playerHudSortHead(\"win\", \"NOW\", \"Current win odds\")"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "elo_delta:[\"Δ\", \"Live Elo position against the living field\", \"numeric\"],"
        ));
        assert_eq!(EMBEDDED_INDEX.matches("--hud-tracks:").count(), 1,
            "the flat track list is declared once and then only written from the column model");
        // A gutter between adjacent figures, and no per-value hairline. The
        // heading and every row keep the same gutters, or the heads skew off
        // their own figures.
        assert_eq!(EMBEDDED_INDEX.matches("column-gap: var(--hud-stat-gap, 3px);").count(), 2,
            "the heading and the rows divide their columns with the same gutters");
        // All three render-time lists must stay in one reading order: the
        // column model lays out and drags the cells, the heading names them,
        // and playerHudStats supplies their figures. Total population comes
        // from the player payload, while food comes from the existing public
        // yield payload rather than a separate request.
        fn assert_hud_stat_order(source: &str, section: &str) {
            let expected = [
                "cities", "population", "food", "production", "science", "culture", "faith", "gold",
                "military", "wonders", "suzerain", "score",
            ];
            let mut cursor = 0;
            for key in expected {
                let needle = format!("\"{key}\"");
                let offset = source[cursor..]
                    .find(&needle)
                    .unwrap_or_else(|| panic!("{section} is missing the {key} statistic"));
                cursor += offset + needle.len();
            }
        }
        let stat_columns = EMBEDDED_INDEX
            .split_once("const PLAYER_HUD_COLUMNS = [")
            .expect("player HUD column model")
            .1
            .split_once("];\n")
            .expect("end of player HUD column model")
            .0;
        assert_hud_stat_order(stat_columns, "player HUD column model");
        let stat_figures = EMBEDDED_INDEX
            .split_once("function playerHudStats(player, rank) {")
            .expect("player HUD stat figures")
            .1
            .split_once("// Rebuilding the ribbon")
            .expect("end of player HUD stat figures")
            .0;
        assert_hud_stat_order(stat_figures, "player HUD stat figures");
        let stat_headers = EMBEDDED_INDEX
            .split_once("const PLAYER_HUD_HEAD_LABELS = {")
            .expect("player HUD head labels")
            .1
            .split_once("};")
            .expect("end of player HUD head labels")
            .0;
        {
            // The labels map keys are bare identifiers, so the order check
            // walks `key:[` anchors rather than quoted keys.
            let expected = [
                "cities", "population", "food", "production", "science", "culture", "faith",
                "gold", "military", "wonders", "suzerain", "score",
            ];
            let mut cursor = 0;
            for key in expected {
                let needle = format!("{key}:[");
                let offset = stat_headers[cursor..]
                    .find(&needle)
                    .unwrap_or_else(|| panic!("player HUD head labels are missing {key}"));
                cursor += offset + needle.len();
            }
        }
        // Player, ELO, ELO delta, the Start/Now pair, AGE and PLAN used to be
        // drawn inside one button spanning six tracks, divided with `subgrid`.
        // Each identity fact is its own cell on the flat tracks now, emitted
        // straight from the visible column list, so any of them can be hidden
        // or stand anywhere in the row — and each still opens the dossier,
        // exactly as the one wide button did.
        assert!(EMBEDDED_INDEX.contains("visibleColumns.map(playerHudColumnHead).join(\"\")"),
            "the heading emits one cell per visible column");
        assert!(EMBEDDED_INDEX.contains("visibleColumns.map(rowCell).join(\"\")"),
            "every row emits one cell per visible column");
        assert!(!EMBEDDED_INDEX.contains("grid-template-columns: subgrid"),
            "no cell spans several tracks any more, so nothing needs subgrid");
        assert!(EMBEDDED_INDEX.contains("class=\"diplomacy-identity\" data-hud-col=\"player\""),
            "each identity fact is its own dossier-opening cell");
        assert!(!EMBEDDED_INDEX.contains("minmax(0, .55fr) minmax(0, .55fr)"),
            "a second copy of the identity ratios is exactly what the flat list replaced");
        // The rows carry a 1px border that says at war or defeated, and
        // both boxes are border-box — so the heading carries a transparent one
        // or it divides two more pixels than a row does and every head sits off
        // its own figures by up to a pixel, in opposite directions at the two
        // ends of the table.
        assert!(EMBEDDED_INDEX.contains(
            "align-items: stretch; column-gap: var(--hud-stat-gap, 3px); padding: 0 4px 0 3px;\n    \
             border: 1px solid transparent;"
        ), "the heading has the row's border box, or the columns skew across the table");
        // For the same reason the heading gives up its inline padding wherever
        // a row does. A breakpoint that moves one without the other reopens
        // the skew it was moved to close.
        assert!(EMBEDDED_INDEX.contains(
            "#playerhud .diplomacy-card, #playerhud .ribbon-stat-heading { padding-inline: 2px; }"
        ), "the heading is a row of the same table and gives up the same pixels");
        assert_eq!(EMBEDDED_INDEX.matches("padding: 1px 4px 1px 3px;").count(), 1,
            "the row's inline padding is written once, and the heading matches it");
        // `clip`, not `ellipsis`: the fitter compares integral scrollWidth with
        // integral clientWidth while the browser applies text-overflow on any
        // sub-pixel overflow, so ellipsis spends a character on a head that
        // renders whole. Measured at 1600px: ELO in its 60px column.
        assert!(EMBEDDED_INDEX.contains(
            "min-width: 0; overflow: hidden; text-overflow: clip; white-space: nowrap;"
        ));

        // One bar per seam between two adjacent data columns, dragged to move
        // width from the column on its left into the column on its right.
        assert!(EMBEDDED_INDEX.contains("const HUD_COLUMN_STORAGE_KEY = \"civvis-hud-columns-v1\";"));
        // Every fact in the player HUD can order its rows. Rank is the first
        // dedicated standings column and is derived from the score standing,
        // while Watch-as and lock cells are actions, deliberately outside the
        // model, so an operator never has to sort a meaningless column just to
        // reach ELO, a yield, or an active plan.
        assert!(EMBEDDED_INDEX.contains(
            "...PLAYER_HUD_COLUMNS.filter(column => !column.fixed).map(column => column.key),"
        ), "every fact is sortable; the fixed Watch-as action is not a fact");
        assert!(EMBEDDED_INDEX.contains(
            "{key:\"rank\", label:\"Score rank\", block:\"identity\", min:\"--hud-rank-min\", width:.7},"
        ), "rank should be a measured, draggable identity column");
        assert!(EMBEDDED_INDEX.contains(
            "{key:\"watch\", label:\"Watch as\", block:\"identity\", min:\"--hud-watch-column\", width:0, fixed:true},"
        ), "Watch-as rides in the model as a reorderable column on a fixed track");
        assert!(EMBEDDED_INDEX.contains("const HUD_SORT_STORAGE_KEY = \"civvis-player-hud-sort-v1\";"));
        assert!(EMBEDDED_INDEX.contains("function playerHudSortValue(player, key, stats, rankById)"));
        assert!(EMBEDDED_INDEX.contains("if (key === \"rank\") return rankById.get(player.id) ?? null;"));
        assert!(EMBEDDED_INDEX.contains(
            "return key === \"rank\" || PLAYER_HUD_TEXT_SORT_COLUMNS.has(key) ? \"asc\" : \"desc\";"
        ), "rank should initially put #1 first");
        assert!(EMBEDDED_INDEX.contains("function sortedPlayerHudPlayers(players, statsByPlayer, rankById)"));
        assert!(EMBEDDED_INDEX.contains("function togglePlayerHudSort(key)"));
        assert!(EMBEDDED_INDEX.contains("if (leftValue === null || rightValue === null) {"));
        assert!(EMBEDDED_INDEX.contains(
            "if (leftValue !== rightValue) return leftValue === null ? 1 : -1;"
        ), "unavailable values stay below observed values in either sort direction");
        assert!(EMBEDDED_INDEX.contains(
            "if (rawValue === null || rawValue === undefined || rawValue === \"\") return null;"
        ), "a missing statistic is distinct from a numeric zero when sorting");
        assert!(EMBEDDED_INDEX.contains("if (key === \"win_start\") return oddsValue(player.odds_start);"));
        assert!(EMBEDDED_INDEX.contains("if (key === \"win_delta\") {"));
        assert!(EMBEDDED_INDEX.contains("return playerHudEloDeltaValue(player);"),
            "Elo delta sorting should use its numeric model value");
        assert!(EMBEDDED_INDEX.contains("return oddsValue(player.odds_now);"),
            "the NOW Win heading should order by its current estimate");
        assert!(EMBEDDED_INDEX.contains("if (delta > .02) return {symbol:\"↗\", direction:\"up\"};"));
        assert!(EMBEDDED_INDEX.contains("if (delta < -.02) return {symbol:\"↘\", direction:\"down\"};"));
        for key in ["rank", "civ", "leader", "player", "elo", "elo_delta", "age", "plan"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("{key}:[")),
                "the {key} heading should carry a sortable label"
            );
        }
        for key in ["win_start", "win_delta", "win"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("playerHudSortHead(\"{key}\"")),
                "the {key} heading should be sortable"
            );
        }
        assert!(EMBEDDED_INDEX.contains(
            "return playerHudSortHead(column.key, label, title, classes, attrs);"
        ), "every generated column heading should be sortable");
        assert!(!EMBEDDED_INDEX.contains("playerHudSortHead(\"watch\""),
            "Watch-as stays an action instead of a sort target");
        assert!(EMBEDDED_INDEX.contains("class=\"hud-sort-head\" data-hud-sort=\"${key}\""));
        assert!(EMBEDDED_INDEX.contains(
            "rank:[\"RANK\", \"Score rank\"],"
        ), "the rank figures need their own named sort header");
        assert!(EMBEDDED_INDEX.contains(
            "minmax(var(--hud-rank-min), .7fr) minmax(var(--hud-ident-min), 2.035fr)"
        ), "the rank heading should occupy its own first identity track");
        assert!(EMBEDDED_INDEX.contains(
            "class=\"diplomacy-rank\" data-hud-col=\"rank\""
        ), "the rank figure and its heading name the same column");
        assert!(EMBEDDED_INDEX.contains("const sort = ev.target.closest?.(\"[data-hud-sort]\");"));
        assert!(EMBEDDED_INDEX.contains("#playerhud .hud-sort-cell[data-hud-sort-active=\"true\"]"));
        assert!(EMBEDDED_INDEX.contains("const scoreRankedMajors = state.players"));
        assert!(EMBEDDED_INDEX.contains(
            "const majors = sortedPlayerHudPlayers(scoreRankedMajors, statsByPlayer, rankById);"
        ));
        assert!(EMBEDDED_INDEX.contains("function playerHudColumnSeams()"));
        assert!(EMBEDDED_INDEX.contains("function aimPlayerHudSeam(seam, targetWidth)"));
        assert!(EMBEDDED_INDEX.contains("class=\"hud-col-grip\" type=\"button\" data-hud-column-seam="));
        assert!(EMBEDDED_INDEX.contains("role=\"separator\" aria-orientation=\"vertical\""));
        // The bar takes the pointer; the layer over the heads does not, or the
        // heads lose their tooltips and the All button loses its click.
        assert!(EMBEDDED_INDEX.contains(
            ".hud-col-grips { position: absolute; inset: 0; z-index: 2; pointer-events: none; }"
        ));
        assert!(EMBEDDED_INDEX.contains("cursor: col-resize; pointer-events: auto; touch-action: none;"));
        // A repaint mid-gesture would take the bar out from under the pointer
        // along with its pointer capture, and the open watch-pace menu is
        // anchored to its select the same way.
        assert!(EMBEDDED_INDEX.contains(
            "if (html === hudHtml || hudLayoutGesture?.name === \"players\" || playerHudColumnGesture\n      || playerHudReorderGesture || hudPaceHeldOpen()) {"
        ));
        assert!(EMBEDDED_INDEX.contains("function hudPaceHeldOpen()"));
        // The moment focus leaves the select, the held snapshots paint.
        assert!(EMBEDDED_INDEX.contains(
            "if (ev.target.closest?.(\"[data-hud-pace]\") && state) drawPlayerHud();"
        ));
        // The bars are placed from the rendered heading, so they cannot drift
        // from the columns they name.
        assert!(EMBEDDED_INDEX.contains("function syncPlayerHudColumnGrips()"));
        assert!(EMBEDDED_INDEX.contains("grip.style.left = `${Math.round(left.right - origin)}px`;"));
        // The fitter has to measure the cell: the figure is centered content in
        // its own grid, so its clientWidth and scrollWidth are always equal and
        // it can never report the overflow that would shrink it.
        assert!(EMBEDDED_INDEX.contains(
            "{selector:\"#playerhud .ribbon-stat b\", min:10, max:15, parentWidth:true},"
        ));
        // No coloured bloom behind eighty figures at once.
        assert!(!EMBEDDED_INDEX.contains("text-shadow: 0 1px 2px #000, 0 0 8px currentColor;"));
        assert!(EMBEDDED_INDEX.contains(
            "class=\"diplomacy-identity diplomacy-civ-link\" data-hud-col=\"civ\" data-hud-action=\"capital\""
        ));
        assert!(!EMBEDDED_INDEX.contains("class=\"empire-link\""));
        assert!(!EMBEDDED_INDEX.contains(">Empire</button>"));
        assert!(!EMBEDDED_INDEX.contains("class=\"capital-link\""));
        assert!(!EMBEDDED_INDEX.contains("data-hud-action=\"empire\""));
        assert!(EMBEDDED_INDEX.contains("function focusCapital(pid)"));
        assert!(EMBEDDED_INDEX.contains("--hud-row-height: 26px;"));
        assert!(EMBEDDED_INDEX.contains("function dismissOverlay(name, source)"));
        assert!(EMBEDDED_INDEX.contains("addEventListener(\"pointerdown\", event =>"));
        assert!(EMBEDDED_INDEX.contains("overlay-return-flash .24s ease-in-out 3"));
        assert!(EMBEDDED_INDEX.contains("restore it in Display Settings"));
        assert_eq!(
            EMBEDDED_INDEX
                .matches("class=\"sidebar-section\"")
                .count(),
            6,
            "every scrolling left-panel section should be collapsible; the \
             lens dock below them carries its own panel style"
        );
        assert!(EMBEDDED_INDEX.contains("function initSidebarSections()"));
        assert!(EMBEDDED_INDEX.contains("civvis-sidebar-sections-v1"));
        // The lens dock is a fixed panel at the deck's lower edge. It shares
        // the saved-disclosure store with the scrolling sections and needs no
        // scroll-into-view choreography — that machinery went with the old
        // in-scroller placement.
        assert!(EMBEDDED_INDEX.contains("#side details[data-section]"));
        assert!(
            !EMBEDDED_INDEX.contains("revealMapLensSection"),
            "a docked lens panel has nothing to scroll into view"
        );
        // Collapsing the command deck collapses the deck alone. Every map
        // overlay is switched from the deck's Display Settings instead, so the
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
                "Display Settings should hide the {element} map overlay"
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
        // The AI strategy dossier speaks for one civilization at a time, and which
        // civilizations it may speak for is the observation's answer, not the
        // panel's: `view_player` names a seat in a played game and in Watch as,
        // and is null only for the view entitled to read every plan.
        assert!(EMBEDDED_INDEX.contains("function strategySeats()"));
        assert!(EMBEDDED_INDEX.contains("if (viewer !== null && viewer !== undefined) \
             return players[viewer] ? [viewer] : [];"));
        assert!(EMBEDDED_INDEX.contains("pick.style.display = seats.length > 1 ? \"block\" : \"none\";"));
        // The observed seat's study rides in `me`; a rival's rides in
        // `players[]`, and only the omniscient view is sent it.
        assert!(EMBEDDED_INDEX
            .contains("const source = state.view_player === p.id ? (state.me || {}) : p;"));
        // The log never reorders: an overflowing log retires its least
        // valuable entry instead of holding important ones frozen at the top.
        assert!(!EMBEDDED_INDEX.contains("e.important && now - e.at < 6000"));
        assert!(EMBEDDED_INDEX.contains("const CAP = 60, FRESH = 12"));
        assert!(EMBEDDED_INDEX.contains("SERVER_EVENT_VALUES"));
        assert!(EMBEDDED_INDEX.contains("const floor = active ? (SPEC ? 32 : 16) : 0"));
        // The repaint rate answers to what a frame actually costs.
        assert!(EMBEDDED_INDEX.contains("const MAX_ANIMATION_PAINT_SHARE = .5"));
        assert!(EMBEDDED_INDEX
            .contains("Math.max(floor, drawCost / MAX_ANIMATION_PAINT_SHARE)"));
        // The command map is the only presentation surface. There is no style
        // selector, persisted mode, cinematic module, or painted atlas path.
        assert!(EMBEDDED_INDEX.contains("const MAP_PROJECTION = 0.92"));
        for removed in [
            "id=\"viewsel\"",
            "const VIEW_MODES",
            "civvis-view-v3",
            "Painted 2D",
            "Cinematic 3D",
            "/cinematic3d.js",
            "globalThis.Cinematic3D",
            "TERRAIN_ATLAS",
            "NATURAL_WONDER_ATLAS",
            "WORLD_WONDER_ATLAS",
            "const MODE =",
            "MODE.painted",
            "cinemaActive",
            "drawCinematic",
            "function stageAttack(",
            "const SHOT_KIND = {",
            "anim.shots",
            "anim.motes",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(removed),
                "obsolete presentation code remains: {removed}"
            );
        }
        // Only what the camera can reach is drawn.
        assert!(EMBEDDED_INDEX.contains("const onscreen = []"));
        // Strategic combat stays diagrammatic: damage labels and compact hit
        // sparks, without a second cinematic effects renderer.
        assert!(EMBEDDED_INDEX
            .contains("anim.floats.push({ x, y: y - 16, txt: \"-\" + dmg"));
        assert!(EMBEDDED_INDEX.contains("anim.sparks.push({ x, y, t0: now });"));
        assert!(EMBEDDED_INDEX.contains(".diplomacy-card.at-war"));
        assert!(EMBEDDED_INDEX.contains("function cameraYBounds"));
        assert!(EMBEDDED_INDEX.contains("cam.y = clampCameraY(cam.y)"));
        assert!(EMBEDDED_INDEX.contains("const focusBounds = mapFocusBounds();"));
        assert!(EMBEDDED_INDEX.contains("return { min:centered, max:centered };"));
        // Flat charts expose both wrapping axes as persistent display choices.
        // East-west starts on, north-south starts off, and both the projected
        // tile positions and canonical hit-test positions consume the choice.
        assert!(EMBEDDED_INDEX.contains("id=\"wrapxchk\" checked"));
        assert!(EMBEDDED_INDEX.contains("id=\"wrapychk\""));
        assert!(EMBEDDED_INDEX.contains(
            "let FLAT_MAP_WRAP_X = localStorage.getItem(\"civvis-flat-map-wrap-x\") !== \"0\";"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "let FLAT_MAP_WRAP_Y = localStorage.getItem(\"civvis-flat-map-wrap-y\") === \"1\";"
        ));
        assert!(EMBEDDED_INDEX.contains("function mapWrapsY()"));
        assert!(EMBEDDED_INDEX.contains("function wrapY(y, about = null)"));
        assert!(EMBEDDED_INDEX.contains("const wy = wrapY(S * 1.5 * r) - cam.y;"));
        assert!(EMBEDDED_INDEX.contains(
            "return canonicalOffsetPos(p, mapWrapsX(), mapWrapsY());"
        ));
        assert!(EMBEDDED_INDEX.contains("setFlatMapWrap(\"x\", wrapXBox.checked)"));
        assert!(EMBEDDED_INDEX.contains("setFlatMapWrap(\"y\", wrapYBox.checked)"));

        // Default camera moves compose at the exact center of the rectangle
        // requested by the operator: the command deck's right edge to the
        // victory rail's left edge, and the player HUD's bottom edge to the
        // screen bottom. At the responsive breakpoint the victory rail becomes
        // a top band, so its live box extends the top edge instead of collapsing
        // the horizontal stage against its 8px left gutter. A missing widget
        // naturally leaves its screen edge in place, and the minimap is
        // deliberately absent from this calculation.
        //
        // The measurement takes the box it is asked about, because the map
        // area's automatic fit asks it of the whole container while the camera
        // asks it of the viewport that fit produced. One rule, two questions —
        // see `the_map_area_is_a_rectangle_the_viewer_can_set`.
        assert!(EMBEDDED_INDEX.contains("function mapOverlayVisible(name)"));
        assert!(EMBEDDED_INDEX.contains(
            "document.body.classList.contains(\"sidebar-hidden\")"
        ));
        assert!(EMBEDDED_INDEX.contains("function mapWidgetBox(name, origin)"));
        assert!(EMBEDDED_INDEX.contains("function mapFocusBounds()"));
        assert!(EMBEDDED_INDEX.contains("function mapFocusPoint()"));
        assert!(EMBEDDED_INDEX.contains(
            "left = Math.max(0, Math.min(width, sideRect.right - origin.left));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (players) top = Math.max(0, Math.min(height, players.bottom));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const spansWidth = victory.left <= 16 && victory.right >= width - 16;"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (spansWidth) top = Math.max(top, Math.max(0, Math.min(height, victory.bottom)));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "else right = Math.max(0, Math.min(width, victory.left));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (right <= left) { left = 0; right = width; }"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (bottom <= top) { top = 0; bottom = height; }"
        ));
        assert!(!EMBEDDED_INDEX.contains(
            "if (minimap) left = Math.max(left, (minimap.left + minimap.right) / 2);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "return {x:(bounds.left + bounds.right) / 2, y:(bounds.top + bounds.bottom) / 2};"
        ));
        assert!(EMBEDDED_INDEX.contains("function reframeIfMapFocusBoundsChanged("));
        assert!(EMBEDDED_INDEX.contains(
            "reframeIfMapFocusBoundsChanged(priorBounds, priorFocus);"
        ));
        assert!(EMBEDDED_INDEX.contains("function cameraCenterForWorld("));
        assert!(EMBEDDED_INDEX.contains("const actualScale = Math.max(.01, scale);"));
        assert!(EMBEDDED_INDEX.contains("function currentMapFocusWorld()"));
        assert!(EMBEDDED_INDEX.contains("function reframeCurrentMapFocus(world)"));
        assert!(EMBEDDED_INDEX.contains(
            "const {x:desiredX, y:desiredY} = mapFocusPoint();"
        ));
        // The domination column counts captured capitals, and says so. "HQs"
        // was a word from no part of this game.
        assert!(EMBEDDED_INDEX.contains("<span></span><span>Capitals</span>"));
        assert!(!EMBEDDED_INDEX.contains("HQs"));
        assert!(EMBEDDED_INDEX.contains("<span>Watch as</span>"));
        assert_eq!(
            EMBEDDED_INDEX
                .matches("Spectator - Full Map Visablity")
                .count(),
            2,
            "the initial and refreshed viewpoint menus should use the same spectator label"
        );
        assert!(EMBEDDED_INDEX.contains(
            "Player ${p.id + 1} - ${p.civ} (${p.leader || \"Unknown leader\"})"
        ));
        assert!(EMBEDDED_INDEX.contains("id=\"viewplayer\""));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/view\""));
        // The ribbon repaints under the cursor, so its buttons declare their
        // action as data and one delegated listener dispatches it.
        assert!(EMBEDDED_INDEX.contains(
            "class=\"watch-as-link\" data-hud-col=\"watch\" data-hud-action=\"watch\""
        ));
        assert!(EMBEDDED_INDEX.contains(">Watch as</button>"));
        // The label is centred against a border rather than ellipsized, so the
        // fitter is asked for a few pixels back: fitted to the last pixel, its
        // own rounding tolerance let the "s" of "Watch as" sit on the frame.
        assert!(EMBEDDED_INDEX
            .contains("{selector:\"#playerhud .watch-as-link\", min:9, max:12, slack:6}"));
        assert!(EMBEDDED_INDEX.contains(
            "class=\"spectator-view-link\" data-hud-action=\"spectator\""
        ));
        assert!(EMBEDDED_INDEX.contains(
            "Spectator mode: see everyone with full map visibility"
        ));
        assert!(EMBEDDED_INDEX.contains(">All</button>"));
        assert!(EMBEDDED_INDEX.contains("data-hud-action=\"watch\" data-hud-civ=\"${p.id}\""));
        assert!(EMBEDDED_INDEX.contains("data-hud-action=\"dossier\" data-hud-civ=\"${p.id}\""));
        assert!(EMBEDDED_INDEX.contains("spectatePlayer(null);"));
        assert!(EMBEDDED_INDEX.contains("else spectatePlayer(id);"));
        assert!(EMBEDDED_INDEX.contains("async function spectatePlayer(player)"));
        // Watching one civilization is a persistent empire portrait, not a
        // one-time jump to its capital. Borders/cities define the durable
        // frame; strategic, grouped, promoted, and war-front units may widen
        // it, while a lone recon unit cannot continually zoom the map out.
        assert!(EMBEDDED_INDEX.contains("function watchedEmpireSubjects(player)"));
        assert!(EMBEDDED_INDEX.contains(
            "function observedViewGoal(anchors, oneEmpire = Number.isInteger(state?.view_player))"
        ));
        assert!(EMBEDDED_INDEX.contains("watchedEmpireAutoFrame"));
        // The same portrait on a round world. Framing the whole globe is the
        // right shot only when the whole of it is being watched: a seat has
        // seen the ground it walked and nothing else, so a globe-wide opening
        // frame left a new single-player game staring at blank ocean with its
        // own settler nowhere on the stage.
        let observed_view = EMBEDDED_INDEX
            .split("function setObservedPlayersView(smooth = false)")
            .nth(1)
            .unwrap()
            .split("function mainLandmassAnchor()")
            .next()
            .unwrap();
        assert!(observed_view.contains("if (planetMap() && !watched) { fitPlanetView(); return; }"));
        assert!(observed_view.contains("if (planetMap()) skyReturnHome();"));
        assert!(EMBEDDED_INDEX.contains("function observedCameraPoints(anchors)"));
        assert!(EMBEDDED_INDEX.contains("function observedPlanetViewGoal(anchors, maximum)"));
        assert!(EMBEDDED_INDEX
            .contains("if (planetMap()) return observedPlanetViewGoal(anchors, maximum);"));
        // A far-flung scout must not zoom the empire shot out past the world
        // itself and off into the system.
        assert!(EMBEDDED_INDEX
            .contains("planetScaleClamp(Math.max(wholeWorld, Math.min(maximum, fitX, fitY)))"));
        assert!(EMBEDDED_INDEX.contains("const EMPIRE_RECON_UNITS"));
        assert!(EMBEDDED_INDEX.contains("const atWarFront"));
        assert!(EMBEDDED_INDEX.contains("Number(unit.formation) > 0"));
        assert!(EMBEDDED_INDEX.contains("Number(unit.level) >= 3"));
        assert!(EMBEDDED_INDEX.contains("player log"));
        assert!(EMBEDDED_INDEX.contains("Spectator · combined summary"));
        assert!(EMBEDDED_INDEX.contains("let eventLogs = new Map()"));
        assert!(EMBEDDED_INDEX.contains("function chronicleWorldEvents(next)"));
        // The war log reads the engine's ledger straight out of the
        // observation, so the panel and its source must ship together.
        assert!(EMBEDDED_INDEX.contains("function drawWarLog()"));
        assert!(EMBEDDED_INDEX.contains("function warsForLog(wars)"));
        assert!(EMBEDDED_INDEX.contains("id=\"warsec\""));
        assert!(EMBEDDED_INDEX.contains("function warTheaterSubjects(war)"));
        assert!(EMBEDDED_INDEX.contains("function focusWarOnMap(warKey)"));
        assert!(EMBEDDED_INDEX.contains("for (const site of war.theater || []) add(site?.pos);"));
        assert!(EMBEDDED_INDEX.contains("data-war-key=\"${escapeAttr(warLogKey(war))}\""));
        assert!(EMBEDDED_INDEX.contains(">${mapAction}</button></div>"));
        // Watching is a held reading position as well as a camera move: the
        // control is a toggle, it names the state it is in, and it reports that
        // state to assistive technology.
        assert!(EMBEDDED_INDEX.contains(
            "const mapAction = watched ? \"Watching\" : (over ? \"View aftermath\" : \"Watch\");"
        ));
        assert!(EMBEDDED_INDEX.contains("aria-pressed=\"${watched ? \"true\" : \"false\"}\""));
        assert!(EMBEDDED_INDEX.contains("${watched ? \" watched\" : \"\"}"));
        assert!(EMBEDDED_INDEX.contains("${watched ? \" watching\" : \"\"}"));
        assert!(EMBEDDED_INDEX.contains("function watchWar(warKey)"));
        assert!(EMBEDDED_INDEX.contains("function releaseWatchedWar()"));
        // The whole point of the hold: the watched card keeps the offset it had
        // inside the list's viewport while the rest of the chronicle re-sorts
        // around it, and the scroll pays for the difference.
        assert!(EMBEDDED_INDEX.contains("function measureWatchedWar(el)"));
        assert!(EMBEDDED_INDEX.contains("function holdWatchedWar(el)"));
        assert!(EMBEDDED_INDEX.contains("function warCardFor(el, warKey)"));
        let war_hold = EMBEDDED_INDEX
            .split("function holdWatchedWar(el)")
            .nth(1)
            .unwrap()
            .split("function watchWar(warKey)")
            .next()
            .unwrap();
        assert!(war_hold.contains(
            "const within = card.getBoundingClientRect().top - el.getBoundingClientRect().top + el.scrollTop;"
        ));
        assert!(war_hold.contains("const target = Math.max(0, Math.min(reach, within - watchedWar.offset));"));
        assert!(war_hold.contains("el.scrollTop = target;"));
        // Re-measured before every rebuild, so a viewer who scrolls the log
        // moves the lock rather than fighting it, and the sort order itself is
        // never rewritten to keep the card still.
        let war_draw = EMBEDDED_INDEX
            .split("function drawWarLog()")
            .nth(1)
            .unwrap()
            .split("function drawSide(")
            .next()
            .unwrap();
        assert!(war_draw.contains("measureWatchedWar(el);"));
        assert!(war_draw.contains("holdWatchedWar(el);"));
        assert!(war_draw.contains("if (watchedWar && watchedWar.seed !== state.seed) watchedWar = null;"));
        assert!(war_draw.contains("if (watchedWar && !warCardFor(el, watchedWar.key)) watchedWar = null;"));
        assert!(
            !war_draw.contains("wars.sort("),
            "the war log must stay the engine's chronicle; a hold moves the scroll, not the order"
        );
        assert!(EMBEDDED_INDEX
            .contains("if (watchedWar && watchedWar.key === key) releaseWatchedWar();"));
        assert!(EMBEDDED_INDEX.contains(".war-card.watched { border-color: #d8ad5e;"));
        assert!(
            EMBEDDED_INDEX.find(".war-card.ended .war-period").unwrap()
                < EMBEDDED_INDEX.find(".war-card.watched .war-period").unwrap(),
            "the watched rules share `.ended`'s specificity and must follow it to win"
        );
        let war_focus = EMBEDDED_INDEX
            .split("function focusWarOnMap(warKey)")
            .nth(1)
            .unwrap()
            .split("function drawWarLog()")
            .next()
            .unwrap();
        assert!(war_focus.contains("takeCameraControl();"));
        assert!(war_focus.contains("flyCameraTo(subjects[subjects.length - 1].pos"));
        assert!(war_focus.contains("const goal = observedViewGoal(subjects, true);"));
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
        // Both bars grow outward from the seam between the columns: each is
        // pushed against it, squared off on that end and rounded on the other,
        // and the seam itself is drawn as an axis under a full-height rule.
        assert!(EMBEDDED_INDEX.contains(
            ".war-side.aggressor .war-belligerent-bar { margin-left: auto; border-radius: 2px 0 0 2px; }"
        ));
        assert!(EMBEDDED_INDEX.contains(
            ".war-side.defender .war-belligerent-bar { margin-right: auto; border-radius: 0 2px 2px 0; }"
        ));
        assert!(EMBEDDED_INDEX
            .contains(".war-columns::before { content: \"\"; position: absolute; top: 0; bottom: 0; left: 50%;"));
        // Painted over the bars, so two sides of one row cannot fuse into a
        // single two-tone bar with no visible origin between them.
        assert!(EMBEDDED_INDEX.contains("box-shadow: 0 0 0 1px #0c110f"));
        assert!(EMBEDDED_INDEX.contains("min-width: 3px; height: 4px"));
        // The busiest army in a war tops out well short of its column's rim.
        assert!(EMBEDDED_INDEX.contains("const WAR_EFFORT_MAX_PERCENT = 80;"));
        assert!(EMBEDDED_INDEX.contains("const effort = WAR_EFFORT_MAX_PERCENT * share;"));
        assert!(EMBEDDED_INDEX.contains("measured out from the centre"));
        assert!(!EMBEDDED_INDEX.contains("strength_total"));
        assert!(!EMBEDDED_INDEX.contains("Military strength at entry"));
        assert!(EMBEDDED_INDEX.contains("war-row-label\">Chronology"));
        assert!(EMBEDDED_INDEX.contains("war-row-label\">Losses"));
        assert!(EMBEDDED_INDEX.contains("Peace deal terms"));
        let belligerents = EMBEDDED_INDEX.find("war-row-label\">Belligerents").unwrap();
        let losses = EMBEDDED_INDEX.find("war-row-label\">Losses").unwrap();
        let chronology = EMBEDDED_INDEX.find("war-row-label\">Chronology").unwrap();
        assert!(belligerents < losses && losses < chronology);
        // A belligerent gets one section carrying its whole involvement, so the
        // note is built from that belligerent's intervals rather than from one
        // row per interval, and a second entry reads as a re-entry.
        assert!(EMBEDDED_INDEX.contains("entered ${turn}${interval.entered}"));
        assert!(EMBEDDED_INDEX.contains("re-entered ${turn}${interval.entered}"));
        assert!(EMBEDDED_INDEX
            .contains("peaced out ${notes.length ? \"\" : \"Turn \"}${interval.exited}"));
        assert!(EMBEDDED_INDEX.contains("function warMergeParties(parties)"));
        assert!(EMBEDDED_INDEX.contains("function warPartyIntervals(party)"));
        // The bar rules the line under the name it measures, so the name comes
        // first in the row and the bar follows it.
        let belligerent_row = EMBEDDED_INDEX
            .split("function warBelligerentRows(")
            .nth(1)
            .unwrap()
            .split("function nuclearStrikeFor(")
            .next()
            .unwrap();
        let party_name = belligerent_row.find("class=\"war-party-name\"").unwrap();
        let bar = belligerent_row.find("class=\"war-belligerent-bar\"").unwrap();
        let note = belligerent_row.find("class=\"war-party-note\"").unwrap();
        assert!(
            party_name < bar && bar < note,
            "the effort bar belongs below the belligerent's name, above its notes"
        );
        // The declarer is named in one word so the label shares the name's
        // line: a two-line name box drops this side's effort bar below its
        // opposite number's, and the bars are what the two columns compare.
        // Asserted against the emitted markup rather than the whole function,
        // so the comment above the template is still free to name the form it
        // replaced -- a bare `contains` over the body matches the prose too.
        assert!(belligerent_row.contains("class=\"war-party-role\">(aggressor)</span>"));
        assert!(!belligerent_row.contains("class=\"war-party-role\">(initial"));
        assert!(EMBEDDED_INDEX.contains(".war-party-role {")
            && EMBEDDED_INDEX
                .split(".war-party-role {")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap()
                .contains("white-space: nowrap"));
        assert!(belligerent_row.contains("party.player === war.aggressor"));
        // Cities are listed under the belligerent that lost them, said plainly,
        // and ranked capital first then by the population that changed hands.
        assert!(EMBEDDED_INDEX.contains("function warCityLosses(party, war)"));
        assert!(EMBEDDED_INDEX.contains("loss.razed ? \"razed\" : \"conquered\""));
        assert!(EMBEDDED_INDEX
            .contains("Number(b.capital) - Number(a.capital) || b.pop - a.pop"));
        // The class the ledger is ordered by is reachable on the row, never a
        // banner across it.
        assert!(!EMBEDDED_INDEX.contains("war-loss-category"));
        // Each side's casualties are one column packed from the top, never a
        // shared row per unit kind: a side is a coalition, so a row that put
        // one alliance's Warriors opposite another's invited a head-to-head
        // reading of two civilizations that may never have met in the field.
        assert!(EMBEDDED_INDEX.contains("function warLossSideColumn(side, war)"));
        assert!(!EMBEDDED_INDEX.contains("war-loss-unit-row"));
        assert!(!EMBEDDED_INDEX.contains("war-loss-unit-cell empty"));
        // And a belligerent that lost neither a unit nor a city is left out of
        // the ledger entirely. Belligerents above it is where "who fought" is
        // answered; here the column is spent on what was actually lost.
        assert!(EMBEDDED_INDEX.contains("function warPartyLostAnything(party, war)"));
        assert!(EMBEDDED_INDEX
            .contains("warParties(war, declarerSide).filter(party => warPartyLostAnything(party, war))"));
        assert!(EMBEDDED_INDEX.contains("No losses"));
        assert!(EMBEDDED_INDEX.contains("No recorded losses"));
        assert!(EMBEDDED_INDEX.contains("sort((a, b) => a.turn - b.turn)"));
        assert!(EMBEDDED_INDEX.contains("built the world's first"));
        assert!(EMBEDDED_INDEX.contains("changed government from"));
        assert!(!EMBEDDED_INDEX.contains("completed its turn"));
        assert!(!EMBEDDED_INDEX
            .contains("civilization${summaries.length === 1 ? \"\" : \"s\"} completed"));
        assert!(EMBEDDED_INDEX.contains("id=\"strategysec\""));
        // AI strategy is no longer withheld from the omniscient spectator.
        // It was, for as long as the panel could only ever speak for `state.me`
        // and above the world there is no single "me"; it now names the
        // civilization it is speaking for, so the one view that can read every
        // plan is the last one that should hide it.
        assert!(
            EMBEDDED_INDEX.contains("document.getElementById(\"strategysec\").style.display = \"block\";"),
            "the active strategy panel is shown in every view"
        );
        assert!(!EMBEDDED_INDEX
            .contains("document.getElementById(\"strategysec\").style.display = fullMapSpectator"));
        assert!(EMBEDDED_INDEX.contains("if (!fullMapSpectator && (SPEC || govs.length"));
        assert!(EMBEDDED_INDEX.contains(".sort((a, b) => b.score - a.score || a.id - b.id)"));
        assert!(EMBEDDED_INDEX.contains(
            "class=\"diplomacy-rank\" data-hud-col=\"rank\" title=\"Score rank ${rank}\">#${rank}"
        ));
        // The sidebar sits left of the map. Match the declaration rather than
        // its formatting, so restyling the block cannot fail the rule.
        let side_rule = EMBEDDED_INDEX
            .split_once("#side {")
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(rule, _)| rule)
            .unwrap_or_default();
        assert!(side_rule.contains("order: -1"));
        assert!(EMBEDDED_INDEX.contains("<strong>${reportedTurn()}</strong>"));
        assert!(!EMBEDDED_INDEX.contains("${state.turn}/${maxTurns}"));
    }

    #[test]
    fn browser_keeps_the_resizable_command_deck_anchored_and_the_player_hud_clear() {
        // The deck's normal desktop declaration remains the first flex track,
        // while the added seam changes only its width/flex basis. This prevents
        // a resize gesture from becoming a draggable panel.
        assert!(EMBEDDED_INDEX.contains(
            "order: -1; width: clamp(220px, 18vw, 332px); min-width: clamp(220px, 18vw, 332px);"
        ));
        assert!(EMBEDDED_INDEX.contains("flex: 0 0 clamp(220px, 18vw, 332px);"));
        assert!(EMBEDDED_INDEX.contains("id=\"side-resize-handle\" type=\"button\" role=\"separator\""));
        assert!(EMBEDDED_INDEX.contains("Resize the command deck from its right edge"));
        assert!(EMBEDDED_INDEX.contains("const SIDEBAR_WIDTH_STORAGE_KEY = \"civvis-sidebar-width-v1\";"));
        assert!(EMBEDDED_INDEX.contains("function setSidebarWidth(width, persist = false)"));
        assert!(EMBEDDED_INDEX.contains("sidebarDeck.classList.add(\"sidebar-width-custom\")"));
        assert!(EMBEDDED_INDEX.contains("function resetSidebarWidth()"));

        // On desktop the flex sibling leaves no overlap; in the compact fixed
        // arrangement the same live rectangle supplies the HUD's safe left
        // anchor. A narrowed map also receives the same full-width HUD topology
        // as a naturally narrow viewport.
        assert!(EMBEDDED_INDEX.contains("function playerHudSidebarInset()"));
        assert!(EMBEDDED_INDEX.contains("area.style.setProperty(\"--player-hud-left\", `${inset}px`);"));
        assert!(EMBEDDED_INDEX.contains(
            "left: max(var(--panel-edge), var(--player-hud-left, 0px));"
        ));
        assert!(EMBEDDED_INDEX.contains("area.classList.toggle(\"player-hud-compact\", width <= PLAYER_HUD_COMPACT_WIDTH);"));
        assert!(EMBEDDED_INDEX.contains("#maparea.player-hud-compact #playerhud"));
        assert!(EMBEDDED_INDEX.contains("maxHeightRatio:.38, avoidsSidebar:true"));
        assert!(EMBEDDED_INDEX.contains(
            "function hudWidgetMinX(config, margin = hudWidgetMargin())"
        ));
    }

    /// Player-row color is reserved for a current war. Friendships and
    /// alliances remain available in the row's text and dossier, but must not
    /// paint green state frames that compete with the red danger signal. The
    /// omniscient spectator uses the public conflict ledger so both sides —
    /// including active allied parties — stay red regardless of whose turn it
    /// is, while a played or Watch-as view remains relative to that seat.
    #[test]
    fn browser_player_rows_use_relationship_color_only_for_active_wars() {
        assert!(!EMBEDDED_INDEX.contains(".diplomacy-card.allied"));
        assert!(!EMBEDDED_INDEX.contains(".diplomacy-card.friend"));
        assert!(!EMBEDDED_INDEX.contains("const relationClass ="));
        assert!(EMBEDDED_INDEX.contains("function activeWarPlayerIds()"));
        assert!(EMBEDDED_INDEX.contains(
            "if (war.ended !== null && war.ended !== undefined) continue;"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (party.exited === null || party.exited === undefined) players.add(party.player);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const activeWarPlayers = SPEC && !Number.isInteger(state.view_player)"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const atWar = activeWarPlayers ? activeWarPlayers.has(p.id) : p.at_war_with_me;"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const stateClass = `${atWar ? \"at-war\" : \"\"} ${p.alive ? \"\" : \"dead\"}`;"
        ));
        assert!(EMBEDDED_INDEX.contains(".diplomacy-card.expanded:not(.at-war)"));
        assert!(EMBEDDED_INDEX.contains(".diplomacy-card:hover:not(.at-war)"));
        assert!(EMBEDDED_INDEX.contains("#playerhud .diplomacy-card.pinned.at-war"));
    }

    /// A defeated civilization all but disappears from the masthead — its row
    /// keeps only a faded, greyed-out line — but the one action left on that
    /// row still has somewhere to go. A capital survives capture with both
    /// `is_capital` and the `original_owner` that founded it (the same pair
    /// Domination Victory counts), so the ground an empire began on outlives
    /// the empire and the link follows it under whoever took it. Before this
    /// the lookup asked only who *owns* a capital, so every eliminated row
    /// rendered its link disabled and the map forgot them entirely.
    #[test]
    fn browser_points_a_defeated_civilizations_empire_link_at_the_capital_it_founded() {
        assert!(EMBEDDED_INDEX.contains("function capitalIndex()"));
        assert!(EMBEDDED_INDEX
            .contains("if (city.owner === city.original_owner) seat.set(city.owner, city);"));
        assert!(EMBEDDED_INDEX.contains(
            "if (!founded.has(city.original_owner)) founded.set(city.original_owner, city);"
        ));
        // Falling order of what is still true: own seat, then a captured
        // capital, then the founding capital in somebody else's hands.
        assert!(EMBEDDED_INDEX.contains(
            "return index.seat.get(pid) || index.held.get(pid) || index.founded.get(pid) || null;"
        ));
        // The row and the click resolve through the same index, so a link that
        // renders enabled can never be a click that does nothing.
        assert!(EMBEDDED_INDEX.contains("const capital = empireCapitalFrom(capitals, p.id);"));
        assert!(EMBEDDED_INDEX.contains("const capital = empireCapital(pid);"));
        assert!(!EMBEDDED_INDEX
            .contains("state.cities.find(city => city.owner === pid && city.is_capital)"));
        // Only a founding capital nobody has ever seen leaves the link dead,
        // which is the one case where there is genuinely nowhere to fly.
        assert!(EMBEDDED_INDEX.contains("${capital ? \"\" : \" disabled\"}"));
        // The jump is not a surprise: the tooltip names the city and says whose
        // hands it is in now.
        assert!(EMBEDDED_INDEX.contains("View ${p.civ}'s first capital, ${capital.name}"));
        assert!(EMBEDDED_INDEX.contains("now held by ${capitalHolder.civ}"));
        // The row itself stays faded. Keeping the link is the whole change;
        // bringing the row back is not.
        assert!(EMBEDDED_INDEX
            .contains(".diplomacy-card.dead { opacity: .42; filter: grayscale(.75); }"));
        assert!(EMBEDDED_INDEX.contains("${p.alive ? \"\" : \"dead\"}"));
    }

    /// The operator intentionally keeps a small, closed action map. These
    /// assertions make every added shortcut an explicit product decision.
    #[test]
    fn browser_key_bindings_match_the_requested_set() {
        for (action, key) in [
            ("NextAction", "1"),
            ("SettlerLens", "2"),
            ("PlaceTack", "3"),
            ("Fortify", "f"),
            ("Alert", "a"),
            ("PreviousCity", "ArrowLeft"),
            ("NextCity", "ArrowRight"),
        ] {
            let row = format!("{{id: \"{action}\", key: \"{key}\"");
            assert!(
                EMBEDDED_INDEX.contains(&row),
                "required {action} is missing from the {key} shortcut"
            );
        }
        let shortcuts = EMBEDDED_INDEX
            .split_once("const CIVVIS_SHORTCUTS = [")
            .and_then(|(_, tail)| {
                tail.split_once("];\n// One lookup per key.")
                    .map(|(rows, _)| rows)
            })
            .expect("the closed shortcut table");
        assert_eq!(shortcuts.matches("{id: \"").count(), 7);
        assert!(!EMBEDDED_INDEX.contains("const CIV6_BINDINGS = ["));
        assert!(!EMBEDDED_INDEX.contains("let altTap"));
        let legend = EMBEDDED_INDEX
            .split_once("<summary>Keyboard shortcuts</summary>")
            .and_then(|(_, tail)| tail.split_once("</details>").map(|(panel, _)| panel))
            .expect("the keyboard shortcut legend");
        assert_eq!(legend.matches("<kbd>").count(), 7);
        assert_eq!(EMBEDDED_INDEX.matches("<kbd>").count(), 7);
        assert!(legend.contains("<span><kbd>3</kbd>Add a map tack</span>"));
        assert!(EMBEDDED_INDEX.contains(
            "return myCities().slice().sort((left, right) => Number(left.id) - Number(right.id));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "? cities.find(city => city.is_capital) || cities[0]"
        ));
        assert!(EMBEDDED_INDEX.contains(": cities[cities.length - 1];"));

        // Movement: a left click only selects, a left drag pans, a secondary
        // press/release moves units, and the middle button centres. This is the
        // event split in Civ 6's shipped WorldInput.lua.
        assert!(!EMBEDDED_INDEX.contains("updateEdgePan"));
        assert!(!EMBEDDED_INDEX.contains("edgepanchk"));
        assert!(!EMBEDDED_INDEX.contains("civvis-edge-pan"));
        assert!(EMBEDDED_INDEX.contains("const MAC_POINTER_PLATFORM = /mac/i.test("));
        assert!(EMBEDDED_INDEX.contains("function isSecondaryMapButton(ev)"));
        assert!(EMBEDDED_INDEX.contains("function issueSelectedUnitOrder(pos)"));
        assert!(EMBEDDED_INDEX.contains("issueSelectedUnitOrder(pos);"));
        assert!(EMBEDDED_INDEX.contains(
            "for (const p of (sel.reachable || [])) hl[key(p)] = 1;"
        ));
        let ordinary_click = EMBEDDED_INDEX
            .split_once("cv.addEventListener(\"click\", ev => {")
            .expect("the map must have an ordinary click handler")
            .1
            .split_once("\nfunction issueSelectedUnitOrder(pos)")
            .expect("the selection handler must end before the movement handler")
            .0;
        assert!(!ordinary_click.contains("move_to"));
        assert!(!ordinary_click.contains("orderTravel("));
        assert!(ordinary_click.contains("const here = state.units.filter"));
        assert!(!EMBEDDED_INDEX.contains(
            "sel = here.find(u => u.moves_left > 0) || here[0]"
        ));
        assert!(EMBEDDED_INDEX.contains("else if (ev.button === 1) {"));
        // macOS Control-click is a platform secondary click; Command belongs to
        // the browser, and never becomes a map binding.
        assert!(EMBEDDED_INDEX.contains(
            "(MAC_POINTER_PLATFORM && ev.button === 0 && ev.ctrlKey)"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "if (ev.metaKey || ev.ctrlKey || ev.altKey || ev.shiftKey) return undefined;"
        ));
    }

    #[test]
    fn strategic_units_use_a_complete_civ6_icon_atlas() {
        assert!(EMBEDDED_CIV6_UNIT_FLAGS.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(EMBEDDED_CIV6_UNIT_FLAGS.len() > 50_000);
        assert!(
            EMBEDDED_INDEX.contains("CIV6_UNIT_ICON_ATLAS.src = \"/assets/civ6-unit-flags.png\"")
        );

        let ids = EMBEDDED_INDEX
            .split("const CIV6_UNIT_ICON_TYPES = [")
            .nth(1)
            .and_then(|tail| tail.split("];\nconst CIV6_UNIT_ICON_INDEX").next())
            .expect("ordered Civilization VI unit icon IDs");
        let rules = crate::rules::Rules::embedded();
        assert_eq!(ids.matches('"').count() / 2, rules.units.len());
        for unit in rules.units.keys() {
            assert!(
                ids.contains(&format!("\"{unit}\"")),
                "unit {unit} has no Civilization VI icon cell"
            );
        }

        let renderer = EMBEDDED_INDEX
            .split("function drawUnitPictogram")
            .nth(1)
            .expect("strategic unit pictogram renderer");
        assert!(renderer.contains("const official = civ6UnitIconSprite(type, color)"));
        assert!(renderer.contains("cx.drawImage(official"));
        assert!(EMBEDDED_INDEX.contains("const COMMAND_UNIT_ICON_K = () => 1 + .32"));
        assert!(EMBEDDED_INDEX.contains("drawUnitPictogram(u.type, x, y,"));
        assert!(EMBEDDED_INDEX.contains("drawUnitPictogram(unit.type, ux, uy,"));
        assert!(EMBEDDED_INDEX.contains("drawUnitPictogram(d.type, d.x, d.y,"));
        assert!(!EMBEDDED_INDEX.contains("embarked ? \"galley\""));
        assert!(EMBEDDED_INDEX.contains("rr * 1.45 * COMMAND_UNIT_ICON_K(), tokenInk"));
    }

    #[test]
    fn unit_health_bars_only_label_damage() {
        let renderer = EMBEDDED_INDEX
            .split("function drawStrategicUnitHealth")
            .nth(1)
            .and_then(|tail| tail.split("// The Civ VI atlas cells").next())
            .expect("strategic unit health renderer");
        assert!(EMBEDDED_INDEX.contains(
            "const CAPTURE_ONLY_CIVILIAN_UNITS = new Set([\"settler\", \"builder\"]);"
        ));
        assert!(EMBEDDED_INDEX.contains("function unitHasHealth(unit) {"));
        assert!(renderer.contains("if (!Number.isFinite(hp)) return;"));
        assert!(renderer.contains("Math.round(hp)"));
        assert!(renderer.contains("health >= 100"));
        assert!(renderer.contains("cx.strokeText(String(health), x, by + bh / 2)"));
        assert!(renderer.contains("cx.fillText(String(health), x, by + bh / 2)"));
        assert!(EMBEDDED_INDEX.contains(
            "const status = unitHasHealth(u) ? `hp ${u.hp}` : \"capturable\";"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const unitStatus = unitHasHealth(unit) ? `${fmtYield(unit.hp)} HP` : \"capturable\";"
        ));
        assert_eq!(
            EMBEDDED_INDEX.matches("drawStrategicUnitHealth(").count(),
            3,
            "the shared damage-only health renderer should serve the flat map and globe"
        );
    }

    #[test]
    fn city_icons_are_shared_by_flat_globe_and_minimaps() {
        let icon = EMBEDDED_INDEX
            .split("function drawCityIcon")
            .nth(1)
            .and_then(|tail| tail.split("function drawSettlement").next())
            .expect("shared city icon renderer");
        assert!(icon.contains(
            "const base = y + r * .45, wallTop = y + r * .06;"
        ));
        assert!(icon.contains("context.moveTo(x - r * .72, base);"));
        assert!(icon.contains("capital && r >= 2.6"));
        for call in [
            "drawCityIcon(cx, cell.center.x, cell.center.y, r, bannerColor,",
            "drawCityIcon(cx, x, y + cityIconRadius * .16, cityIconRadius,",
            "drawCityIcon(mx2, x, y, cityRadius, cityBannerColor(city.owner),",
            "drawCityIcon(mx2, x, y, citySize * (cityState ? .55 : .62),",
        ] {
            assert!(EMBEDDED_INDEX.contains(call), "missing city icon path: {call}");
        }
    }

    #[test]
    fn empire_city_names_use_the_light_jersey_lane() {
        assert!(EMBEDDED_INDEX.contains(
            "function cityNameInk(owner, fallback) {\n  return mapLens === \"empire\" ? jerseyLanes(owner)[1] : fallback;\n}"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "cityNameInk(city.owner, cityState ? \"#d8ddd9\" : \"#fff4d4\")"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "cityNameInk(c.owner, minorPlate ? \"#d8ddd9\" : \"#fff3cf\")"
        ));
    }

    #[test]
    fn undiscovered_ground_is_an_illustrated_fog_safe_chart() {
        assert!(EMBEDDED_HIDDEN_MAP_MONSTERS.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(EMBEDDED_HIDDEN_MAP_MONSTERS.len() > 500_000);
        assert!(EMBEDDED_HIDDEN_MAP_MONSTERS.len() < 1_000_000);
        assert_eq!(
            u32::from_be_bytes(EMBEDDED_HIDDEN_MAP_MONSTERS[16..20].try_into().unwrap()),
            1536
        );
        assert_eq!(
            u32::from_be_bytes(EMBEDDED_HIDDEN_MAP_MONSTERS[20..24].try_into().unwrap()),
            1280
        );
        assert!(EMBEDDED_INDEX
            .contains("HIDDEN_MAP_MONSTER_ATLAS.src = \"/assets/hidden-map-monsters.png\""));
        assert!(EMBEDDED_INDEX.contains("HIDDEN_MAP_MONSTER_CELL = 256"));
        assert!(EMBEDDED_INDEX.contains("HIDDEN_MAP_MONSTER_VARIANTS = 30"));

        let parchment = EMBEDDED_INDEX
            .split("function drawHiddenMapParchment")
            .nth(1)
            .and_then(|tail| tail.split("function drawHiddenMapMonsters").next())
            .expect("continuous hidden-map parchment renderer");
        assert!(parchment.contains("for (const cell of layer.cells) appendHexPath"));
        assert!(parchment.contains("cx.fillStyle = PARCH; cx.fill()"));
        assert!(!parchment.contains("PARCH_GRID"), "the hidden sheet must not expose a hex grid");

        let monsters = EMBEDDED_INDEX
            .split("function drawHiddenMapMonsters")
            .nth(1)
            .and_then(|tail| tail.split("function drawHiddenMapFrontier").next())
            .expect("hidden-map monster placement");
        assert!(monsters.contains("hiddenMapIsDeep"));
        assert!(monsters.contains("hiddenMapMonsterSeat"));
        assert!(!monsters.contains("strideColumns"));
        assert!(!monsters.contains("cam.scale"), "zoom must not thin stable tale seats");
        assert!(!monsters.contains("const lod"));
        assert!(monsters.contains("cell.x +"));
        assert!(monsters.contains("cell.y +"));
        assert!(monsters.contains("HIDDEN_MAP_TALE_SIZE_MIN"));
        assert!(monsters.contains("HIDDEN_MAP_TALE_SIZE_RANGE"));
        assert!(monsters.contains("HIDDEN_MAP_MONSTER_VARIANTS"));
        assert!(monsters.contains(".26"));

        let seating = EMBEDDED_INDEX
            .split("function hiddenMapMonsterSeat")
            .nth(1)
            .and_then(|tail| tail.split("function drawHiddenMapMonster").next())
            .expect("minimum-distance hidden-map monster seating");
        assert!(seating.contains("radius = 8"));
        assert!(seating.contains("priority >= .04"));
        assert!(seating.contains("hiddenMapMonsterPriority"));
        assert!(seating.contains("other < priority"));
        let viewport = EMBEDDED_INDEX
            .split("function hiddenMapViewport")
            .nth(1)
            .and_then(|tail| tail.split("function hiddenMapSeedWords").next())
            .expect("hidden-map viewport and stable large-tale margin");
        assert!(viewport.contains("monsterCells"));
        assert!(viewport.contains("HIDDEN_MAP_TALE_REACH"));
        let planet_tales = EMBEDDED_INDEX
            .split("function drawPlanetChartMarginalia")
            .nth(1)
            .and_then(|tail| tail.split("function drawPlanetMap").next())
            .expect("pre-globe chart marginalia");
        assert!(planet_tales.contains("candidates.slice(0, 1)"));
        assert!(planet_tales.contains("const size = 3 *"));
        assert!(EMBEDDED_INDEX.contains("drawHiddenMapParchment(hiddenMap);\n  drawHiddenMapMonsters(hiddenMap);"));
        assert!(EMBEDDED_INDEX.contains("drawHiddenMapFrontier(tiles);"));
        assert!(EMBEDDED_INDEX.contains("if (camera.chart && !spectator)"));
    }

    /// Fog of war must not report where the stored map stops. A civilization
    /// that has seen eighteen tiles knows eighteen tiles, and the shape of the
    /// rectangle they are stored in is not one of the things it knows.
    #[test]
    fn the_map_edge_is_veiled_until_a_world_has_been_measured() {
        // The veil is the observed civilization's, not the viewer's: a
        // spectator is not exploring anything, and `wentAround` already
        // answers true for one, so the exhibition keeps the whole rectangle.
        assert!(EMBEDDED_INDEX.contains("function mapEdgeVeiled()"));
        assert!(EMBEDDED_INDEX
            .contains("return !!state && !planetMap() && !wentAround();"));
        // Three to fifteen tiles, per world and per side, so the give is never
        // the same twice and the two sides of one map never agree.
        assert!(EMBEDDED_INDEX.contains("const MAP_VEIL_MIN = 3, MAP_VEIL_MAX = 15;"));
        assert!(EMBEDDED_INDEX.contains("function mapEdgeVeil()"));
        assert!(EMBEDDED_INDEX.contains(
            "MAP_VEIL = {left:reach(0), right:reach(1), top:reach(2), bottom:reach(3)};"
        ));

        // The parchment runs past the stored rectangle rather than stopping on
        // it, and it is drawn out to the frame on every side rather than out to
        // the veil, so no zoom and no bearing can find the end of the sheet.
        let hidden = EMBEDDED_INDEX
            .split("function hiddenMapCellAt")
            .nth(1)
            .and_then(|tail| tail.split("function hiddenMapSeedWords").next())
            .expect("hidden-map cell test and viewport");
        assert!(
            hidden.contains("col < 0 || col >= state.map.width) return mapEdgeVeiled();"),
            "ground past the stored rectangle must read as unsurveyed, not as nothing"
        );
        assert!(hidden.contains("const veiled = !open && mapEdgeVeiled();"));
        assert!(
            hidden.contains("const span = open || veiled ? framedHexBounds(12)"),
            "a veiled sheet and its large marginalia are spanned by the frame, not by the map's rectangle"
        );
        assert!(hidden.contains("const [x, y] = open || veiled ? hexXYRaw(q, row) : hexXY(q, row);"));
        // Only one copy of a wrapping world is drawn, so the background either
        // side of it is somewhere nobody has been rather than somewhere that
        // does not exist — which is what closes the same leak at full zoom-out.
        assert!(EMBEDDED_INDEX.contains("function veilDrawsGroundAt(q, r)"));
        assert!(hidden.contains("const test = veiled ? (q, r) => !veilDrawsGroundAt(q, r)"));
        // A canvas compound path costs quadratic time in its own size, so a
        // sheet that covers a whole frame has to be drawn as its rectangle.
        assert!(hidden.contains("return {open, solid:veiled, cells, monsterCells, test};"));
        assert!(EMBEDDED_INDEX
            .contains("if (layer.solid) cx.rect(minX, minY, maxX - minX, maxY - minY);"));

        // A camera that stops dead on the last row says the same thing the
        // parchment no longer does, so the veil owns that axis while it lasts.
        assert!(EMBEDDED_INDEX.contains("function mapVeilPanBounds()"));
        let bounds = EMBEDDED_INDEX
            .split("function cameraYBounds")
            .nth(1)
            .and_then(|tail| tail.split("function clampCameraY").next())
            .expect("vertical camera bounds");
        assert!(
            bounds.contains("const veil = mapVeilPanBounds();\n  if (veil?.y) return veil.y;"),
            "the veil must be consulted before the map's own rows frame the camera"
        );
        assert!(EMBEDDED_INDEX.contains("function clampCameraX(x)"));

        // Soft, and bouncy: a drag past the bound is resisted rather than
        // refused, a coast into it stretches and is handed to a spring, and
        // both come home to the bound instead of sailing back over the world.
        assert!(EMBEDDED_INDEX.contains("function cameraRubberBand(value, bounds, give = CAMERA_GIVE())"));
        assert!(EMBEDDED_INDEX.contains("function holdCameraInBounds(x, y)"));
        assert!(EMBEDDED_INDEX.contains("function handOffCameraBounce()"));
        assert!(EMBEDDED_INDEX.contains("function settleCameraBounce(dt)"));
        assert!(EMBEDDED_INDEX.contains("if (settleCameraBounce(dt)) active = true;"));
        assert!(
            EMBEDDED_INDEX.contains("[cam.x, cam.y] = holdCameraInBounds("),
            "the drag paths must go through the rubber band"
        );

        // And the thumbnail, which is the loudest statement of extent there is.
        let mini = EMBEDDED_INDEX
            .split("function miniBounds()")
            .nth(1)
            .and_then(|tail| tail.split("function miniLayout").next())
            .expect("minimap bounds");
        assert!(mini.contains("if ((!chartIsOpen() && !mapEdgeVeiled()) || !tiles?.length) {"));
    }

    #[test]
    fn browser_draws_one_perimeter_around_each_natural_wonder_footprint() {
        let continuation = EMBEDDED_INDEX
            .split("function naturalWonderContinues")
            .nth(1)
            .and_then(|tail| tail.split("function drawNaturalWonderPerimeters").next())
            .expect("natural wonder adjacency rule");
        assert!(continuation.contains("neighbor.feature === tile.feature"));

        let flat = EMBEDDED_INDEX
            .split("function drawNaturalWonderPerimeters")
            .nth(1)
            .and_then(|tail| tail.split("function drawTileYields").next())
            .expect("flat natural wonder perimeter renderer");
        assert!(flat.contains("if (naturalWonderContinues(tile, neighbor)) continue;"));
        assert!(flat.contains("EDGE_CORNERS[side]"));

        let planet = EMBEDDED_INDEX
            .split("function drawPlanetNaturalWonderPerimeters")
            .nth(1)
            .and_then(|tail| tail.split("function planetFeatureGlyph").next())
            .expect("planet natural wonder perimeter renderer");
        assert!(planet.contains("TMAP.get(cell.nbrs[side])"));
        assert!(planet.contains("if (naturalWonderContinues(tile, neighbor)) continue;"));

        let mini = EMBEDDED_INDEX
            .split("function drawMiniNaturalWonderPerimeters")
            .nth(1)
            .and_then(|tail| tail.split("function drawMini()").next())
            .expect("flat minimap natural wonder perimeter renderer");
        assert!(mini.contains("if (naturalWonderContinues(tile, neighbor)) continue;"));

        let planet_mini = EMBEDDED_INDEX
            .split("function drawPlanetMiniNaturalWonderPerimeters")
            .nth(1)
            .and_then(|tail| tail.split("function drawPlanetMini()").next())
            .expect("planet minimap natural wonder perimeter renderer");
        assert!(planet_mini.contains("TMAP.get(cell.nbrs[side])"));
        assert!(planet_mini.contains(
            "if (naturalWonderContinues(tile, neighbor)) continue;"
        ));

        assert_eq!(
            EMBEDDED_INDEX.matches("drawNaturalWonderPerimeters(tiles);").count(),
            1,
            "the flat map must paint the landmark perimeter exactly once"
        );
        assert_eq!(
            EMBEDDED_INDEX
                .matches("drawPlanetNaturalWonderPerimeters(cells);")
                .count(),
            1,
            "the planet map must paint the landmark perimeter exactly once"
        );
        assert_eq!(
            EMBEDDED_INDEX
                .matches("drawMiniNaturalWonderPerimeters(tiles, layout);")
                .count(),
            1,
            "the flat minimap must paint the landmark perimeter exactly once"
        );
        assert_eq!(
            EMBEDDED_INDEX
                .matches("drawPlanetMiniNaturalWonderPerimeters(index.entries, projection);")
                .count(),
            1,
            "the planet minimap must paint the landmark perimeter exactly once"
        );
    }

    /// A selected unit's movement is drawn as the engine's own affordances:
    /// the perimeter of everywhere it can end this turn, and a per-edge arrow
    /// wherever `reach_steps` says the remaining movement crosses. The client
    /// must read the engine's per-edge answer rather than re-derive it — only
    /// the engine knows step costs, rivers, and ZOC — and both boundary
    /// renderers must be built from positions, not surveyed tiles, so the
    /// line keeps reading where the range runs into fog.
    #[test]
    fn browser_draws_the_selected_units_reach_perimeter_and_edge_arrows() {
        // The engine's directional step list is what both renderers consume.
        assert!(EMBEDDED_INDEX.contains("function selReachEdges()"));
        assert!(EMBEDDED_INDEX.contains("(sel && sel.reach_steps) || []"));
        assert!(EMBEDDED_INDEX.contains("region.add(key(sel.pos));"));

        let flat = EMBEDDED_INDEX
            .split("function drawFlatReachPerimeter")
            .nth(1)
            .and_then(|tail| tail.split("function drawFlatMapSearchHighlights").next())
            .expect("flat reach perimeter and arrow renderers");
        assert!(flat.contains("EDGE_CORNERS[side]"));
        assert!(
            flat.contains("const tile = TMAP.get(regionKey);"),
            "the perimeter tolerates region tiles beyond the surveyed map"
        );
        assert!(flat.contains("drawEdgeArrow(x + dx / 2, y + dy / 2, dx, dy,"));

        let planet = EMBEDDED_INDEX
            .split("function drawPlanetReachOverlay")
            .nth(1)
            .and_then(|tail| tail.split("function planetFeatureGlyph").next())
            .expect("planet reach overlay renderer");
        assert!(planet.contains("cell.nbrs[side]"));
        assert!(
            planet.contains("planetCellGeometry({pos}, camera, scale, centerX, centerY)"),
            "the globe projects region cells the frame does not carry"
        );
        assert!(planet.contains("entry.cell.nbrs.indexOf(key(edge.to))"));

        // Each map paints the movement overlay exactly once, after fog and
        // the sight perimeter, so the boundary stays visible over parchment.
        assert_eq!(EMBEDDED_INDEX.matches("drawFlatReachPerimeter();").count(), 1);
        assert_eq!(EMBEDDED_INDEX.matches("drawFlatReachArrows();").count(), 1);
        assert_eq!(
            EMBEDDED_INDEX
                .matches("drawPlanetReachOverlay(cells, camera, scale, centerX, centerY);")
                .count(),
            1
        );
        let after_sight = EMBEDDED_INDEX
            .split("drawFlatVisibilityPerimeter(tiles, visible);")
            .nth(1)
            .expect("the flat sight perimeter is painted");
        assert!(
            after_sight.contains("drawFlatReachPerimeter();"),
            "movement range must paint after the sight perimeter and fog"
        );
        // The arrow glyph itself is shared by both projections and knows the
        // two directional forms: a head across the edge, one back, or both.
        assert!(EMBEDDED_INDEX
            .contains("function drawEdgeArrow(mx, my, dx, dy, out, back, k)"));
    }

    #[test]
    fn browser_minimap_highlights_the_camera_footprint_for_both_map_shapes() {
        assert!(EMBEDDED_INDEX
            .contains("function paintMiniViewportFootprint(points, clip = null)"));
        assert!(EMBEDDED_INDEX
            .contains("mx2.fillStyle = \"rgba(255, 223, 133, .18)\""));

        let flat = EMBEDDED_INDEX
            .split("function drawFlatMiniViewportFootprint")
            .nth(1)
            .and_then(|tail| tail.split("function miniHexPath").next())
            .expect("flat minimap viewport footprint");
        assert!(flat.contains("screenToWorld(sx, sy)"));
        assert!(flat.contains("miniViewportScreenCorners().map"));
        assert!(EMBEDDED_INDEX.contains("drawFlatMiniViewportFootprint(layout);"));

        let planet = EMBEDDED_INDEX
            .split("function drawPlanetMiniViewportFootprint")
            .nth(1)
            .and_then(|tail| tail.split("function planetMiniScale").next())
            .expect("planet minimap viewport footprint");
        assert!(planet.contains("const mainRadius = mainCamera.radius * cam.scale;"));
        assert!(planet.contains("const basis = planetViewBasis(mainCamera);"));
        // The screen boundary is walked densely, not just at the corners: a
        // straight screen edge is a curve on the azimuthal chart.
        assert!(planet.contains("miniViewportBoundaryScreenPoints().map"));
        assert!(planet.contains("azimuthalMiniScreenPoint(point.map(value => value / length), projection)"));
        // A ray past the globe's limb is clamped to the limb rather than
        // dropped, so a zoomed-out footprint keeps its corners.
        assert!(planet.contains("if (distance >= 1 - 1e-6) {"));
        assert!(EMBEDDED_INDEX.contains(
            "drawPlanetMiniViewportFootprint(projection);"
        ));
    }

    #[test]
    fn browser_planet_minimap_uses_a_square_cropped_azimuthal_equidistant_chart() {
        // The chart is azimuthal equidistant about the point facing the
        // viewer, cropped to the minimap's square, and the square's half-side
        // is derived from the share of the sphere the crop must hold.
        assert!(EMBEDDED_INDEX.contains("const AZIMUTHAL_MINI_WORLD_SHARE = 0.8;"));
        assert!(EMBEDDED_INDEX.contains("const AZIMUTHAL_MINI_SQUARE_HALF = (() => {"));
        assert!(EMBEDDED_INDEX.contains("if (cropShare(mid) < AZIMUTHAL_MINI_WORLD_SHARE) low = mid; else high = mid;"));
        assert!(EMBEDDED_INDEX.contains("function azimuthalMiniLocalSphereAt(x0, y0)"));
        assert!(EMBEDDED_INDEX.contains("function azimuthalMiniSphereAt(x, y, projection)"));
        assert!(EMBEDDED_INDEX.contains("function azimuthalMiniScreenPoint(point, projection)"));
        // The elliptic Peirce machinery left with the projection it served.
        assert!(!EMBEDDED_INDEX.contains("peirce"));
        assert!(!EMBEDDED_INDEX.contains("Peirce"));
        // Ground the crop cuts off mid-projection must not spill past the
        // frame: the overlays clip to the same square the raster fills and
        // the frame is stroked around.
        assert!(EMBEDDED_INDEX.contains("function azimuthalMiniSquarePath(projection)"));
        assert!(EMBEDDED_INDEX.contains("azimuthalMiniSquarePath(projection); mx2.clip();"));
        // Minimap clicks invert the same chart the raster paints.
        assert!(EMBEDDED_INDEX.contains("const projection = azimuthalMiniProjection(r.width, r.height);"));
        assert!(EMBEDDED_INDEX.contains("const point = azimuthalMiniSphereAt(x, y, projection);"));
        assert!(EMBEDDED_INDEX.contains("const target = point && planetMiniCellAt(point, planetMiniCellIndex());"));
        assert!(EMBEDDED_INDEX.contains("avoidsSidebar:true, square:true"));
    }

    #[test]
    fn browser_planet_minimap_paints_forward_cell_polygons_not_a_per_pixel_raster() {
        // The drag path redraws the minimap on every pointermove, so the
        // planet chart must stay a few dozen batched polygon fills. The
        // per-pixel inverse raster it replaces ran the cell search ~30k times
        // per redraw (~100ms a frame) and made dragging a planet map an
        // eight-frame slideshow; the inverse is reserved for clicks.
        let planet_mini = EMBEDDED_INDEX
            .split("function drawPlanetMini()")
            .nth(1)
            .and_then(|tail| tail.split("function chartUnrolledU").next())
            .expect("planet minimap renderer");
        assert!(planet_mini.contains("const batches = new Map();"));
        assert!(planet_mini.contains("const point = azimuthalMiniScreenPoint(vertex, projection);"));
        assert!(planet_mini.contains("mx2.fill(); mx2.stroke();"));
        // A canvas fill is superlinear in its path's subpaths, so the ground
        // must stay chunked — one path per colour costs ~10x, one path for
        // everything ~40x.
        assert!(EMBEDDED_INDEX.contains("const AZIMUTHAL_MINI_FILL_CHUNK = 48;"));
        assert!(planet_mini.contains("start += AZIMUTHAL_MINI_FILL_CHUNK"));
        assert!(
            !planet_mini.contains("planetMiniCellAt("),
            "the redraw path must not run the per-pixel cell search"
        );
        assert!(
            !EMBEDDED_INDEX.contains("createImageData"),
            "no per-pixel software raster may return to the viewer"
        );
        // Cells smeared past the crop's corners are culled before batching.
        assert!(planet_mini.contains("Math.SQRT2 * AZIMUTHAL_MINI_SQUARE_HALF + .15"));
        assert!(planet_mini.contains("if (dot3(entry.cell.center, projection.basis.out) < towardFloor) continue;"));
    }

    #[test]
    fn browser_tile_yields_are_numbered_in_centered_semantic_rows() {
        assert!(EMBEDDED_INDEX.contains(
            "[\"science\", \"culture\", \"faith\"],\n  [\"food\", \"production\", \"gold\"],"
        ));

        let renderer = EMBEDDED_INDEX
            .split("function drawTileYields")
            .nth(1)
            .and_then(|tail| tail.split("function tri(").next())
            .expect("tile-yield renderer");
        assert!(renderer.contains(".filter(([, amount]) => amount >= 1)"));
        assert!(renderer.contains("(i - (entries.length - 1) / 2) * step"));
        assert!(renderer.contains("cx.fillStyle = YPIP[kind]"));
        assert!(renderer.contains("cx.fillText(label, px, py + .5)"));
        assert!(renderer.contains("const label = fmtYield(amount)"));
        assert!(!renderer.contains("YICON[kind]"));
        assert!(!renderer.contains("yieldGlyph"));
    }

    #[test]
    fn strategic_volcanic_soil_splats_from_the_volcano_facing_edge() {
        let splat = EMBEDDED_INDEX
            .split("function drawStrategicVolcanicSoil")
            .nth(1)
            .and_then(|tail| tail.split("const STRATEGIC_MOUNTAIN_ICON_SCALE").next())
            .expect("strategic Volcanic Soil renderer");
        assert!(splat.contains("nbrTile(t.pos, DIRS[side])"));
        assert!(splat.contains("neighbor?.feature === \"volcano\""));
        assert!(splat.contains("cx.rotate(Math.atan2(sourceY, sourceX))"));
        assert!(splat.contains("hexPath(x, y, S); cx.clip();"));
        assert!(splat.contains("const soilFan = cx.createLinearGradient(0, 0, S + 2, 0)"));
        assert!(splat.contains("cx.moveTo(S + 2, -S)"));
        assert!(splat.contains("cx.bezierCurveTo(4, -7, 1, -3, -1, 0)"));
        assert!(splat.contains("cx.bezierCurveTo(21, 14, 29, 18, S + 2, S)"));
        assert!(!splat.contains("performance.now"));
        assert!(!splat.contains("requestAnimationFrame"));

        let ground_overlay = EMBEDDED_INDEX
            .split("// Volcanic Soil is part of the ground")
            .nth(1)
            .and_then(|tail| tail.split("// --- hex grid (G)").next())
            .expect("strategic Volcanic Soil ground-overlay pass");
        assert!(ground_overlay.contains("if (t.feature !== \"volcanic_soil\") continue;"));
        assert!(ground_overlay.contains("drawStrategicVolcanicSoil(t, x, y0 - elev(t))"));
        assert!(ground_overlay.contains("hexPath(x, y0 - elev(t))"));

        let decorations = EMBEDDED_INDEX
            .split("// --- decorations")
            .nth(1)
            .expect("strategic decoration pass");
        assert!(!decorations.contains("drawStrategicVolcanicSoil(t, x, y)"));
    }

    #[test]
    fn strategic_mountains_and_volcanoes_use_minimal_vector_glyphs() {
        let icon = EMBEDDED_INDEX
            .split("function drawStrategicMountainIcon")
            .nth(1)
            .and_then(|tail| tail.split("function drawFeatureEffects").next())
            .expect("minimal strategic mountain icon renderer");
        assert!(icon.contains("drawMinimalVolcanoCaldera(x, y, STRATEGIC_MOUNTAIN_ICON_SCALE * size)"));
        assert!(icon.contains("drawMinimalMountainGlyph(x, y, STRATEGIC_MOUNTAIN_ICON_SCALE * size)"));
        assert!(!icon.contains("Atlas"));
        let mountain = EMBEDDED_INDEX
            .split("function drawMinimalMountainGlyph")
            .nth(1)
            .and_then(|tail| tail.split("function drawMinimalVolcanoCaldera").next())
            .expect("minimal mountain silhouette renderer");
        assert!(mountain.contains("const silhouette = () =>"));
        assert!(mountain.contains("cx.fillStyle = \"#8c9991\""));
        assert!(mountain.contains("cx.strokeStyle = \"#263a36\""));
        let volcano = EMBEDDED_INDEX
            .split("function drawMinimalVolcanoCaldera")
            .nth(1)
            .and_then(|tail| tail.split("function drawStrategicMountainIcon").next())
            .expect("minimal volcano caldera renderer");
        assert!(volcano.contains("cx.ellipse(0, 1, 9.5, 5.2"));
        assert!(volcano.contains("if (!ice)"));

        let wonder = EMBEDDED_INDEX
            .split("  volcano(x, y, k, art) {")
            .nth(1)
            .and_then(|tail| tail.split("  ruins(x, y, k, art) {").next())
            .expect("minimal Natural Wonder volcano renderer");
        assert!(wonder.contains("drawMinimalVolcanoCaldera(x, y, k, art.ice, art.tint)"));
        assert!(!wonder.contains("drawStrategicVolcanoAtlas"));

        let effects = EMBEDDED_INDEX
            .split("function drawFeatureEffects")
            .nth(1)
            .and_then(|tail| tail.split("function drawStrategicMarsh").next())
            .expect("volcano feature effects renderer");
        assert!(effects.contains("const erupting = t.volcano_state === 2;"));
        assert!(effects.contains("cx.beginPath(); cx.arc(x, y, 11, 0, Math.PI * 2); cx.stroke();"));
        assert!(effects.contains("cx.arc(x, y - 8, 1.35, 0, Math.PI * 2)"));
    }

    #[test]
    fn mountain_and_volcano_tiles_use_minimal_vector_art() {
        assert!(EMBEDDED_INDEX.contains("const MOUNTAIN_TILE_COLOR = \"#49453e\";"));
        assert!(EMBEDDED_INDEX.contains("const VOLCANO_TILE_COLOR = \"#292421\";"));
        assert!(EMBEDDED_INDEX.contains("tile.feature === \"volcano\""));
        assert!(EMBEDDED_INDEX.contains("tileGroundColor(cell.tile, \"#4b5960\")"));
        assert!(EMBEDDED_INDEX.contains("const base = tileGroundColor(t);"));
        assert!(EMBEDDED_INDEX.contains("tileGroundColor(tile, \"#44545a\")"));
        assert!(EMBEDDED_INDEX.contains("const terrainColor = tileGroundColor(t);"));

        assert!(EMBEDDED_INDEX.contains("const STRATEGIC_MOUNTAIN_ICON_SCALE = 1.04;"));
        assert!(EMBEDDED_INDEX.contains("function drawMinimalMountainGlyph"));
        assert!(EMBEDDED_INDEX.contains("function drawMinimalVolcanoCaldera"));
        assert!(!EMBEDDED_INDEX.contains("MOUNTAIN_ATLAS"));
        assert!(!EMBEDDED_INDEX.contains("drawStrategicAtlasCell"));
        assert!(!EMBEDDED_INDEX.contains("drawStrategicMountainFallback"));
        assert!(EMBEDDED_INDEX.contains("drawStrategicMountainIcon(x, y, true);"));
        assert!(EMBEDDED_INDEX.contains("drawStrategicMountainIcon(x, y, false);"));
        assert!(EMBEDDED_INDEX.contains("drawStrategicMountainIcon(0, 0, true, .78);"));
        assert!(EMBEDDED_INDEX.contains("drawStrategicMountainIcon(0, 0, false, .78);"));
    }

    #[test]
    fn instance_tagged_spectator_url_routes_to_the_embedded_page() {
        assert_eq!(request_path("/"), "/");
        assert_eq!(request_path("/?instance=9232"), "/");
        assert_eq!(request_path("/?instance=9232&game=17"), "/");
        assert_eq!(request_path("/state?instance=9232"), "/state");
    }

    #[test]
    fn native_channel_routes_to_the_embedded_page() {
        assert!(viewer_path("/rust"));
        assert!(viewer_path("/rust/"));
        assert!(viewer_path(request_path("/rust?instance=9232&game=17")));
        assert!(!viewer_path("/wasm"));
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
            session.params.leader_pool,
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
                session.params.leader_pool,
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
        let session = Session::new(params);
        let original_seed = session.game.seed;
        let original_script = session.game.map_script;
        let original_speed = session.game.game_speed;
        let shared = shared_for(session);

        shared.stage_next_game_settings(&json!({
            "num_players": 6,
            "map_script": "continents",
            "game_speed": "quick",
            "leader_pool": "historical",
            "victory_conditions": {"culture": false, "score": false},
        }));

        {
            let live = shared.session.lock().unwrap();
            assert_eq!(live.game.seed, original_seed);
            assert_eq!(live.game.map_script, original_script);
            assert_eq!(live.game.game_speed, original_speed);
        }
        assert_eq!(
            decorated_state(&shared)["next_game_settings"],
            json!({
                "seed": 1,
                "players": 6,
                "width": 74,
                "height": 46,
                "city_states": 9,
                "turns": 330,
                "base_ruleset": "civ6",
                "start_era": "ancient",
                "future_era": "classic",
                "turn_structure": "sequential",
                "map": "continents",
                "shape": "flat",
                "poles": "poles",
                "speed": "quick",
                "leader_pool": "historical",
                "teams": [],
                "victories": ["science", "religious", "diplomatic", "domination"],
            })
        );

        let queued = shared.take_next_game_params();
        shared
            .session
            .lock()
            .unwrap()
            .start_automatic_next_game(queued);

        let session = shared.session.lock().unwrap();
        assert_ne!(session.game.seed, original_seed);
        assert_eq!(session.params.num_players, 6);
        assert_eq!(session.params.map_script, MapScript::Continents);
        assert_eq!(session.params.game_speed, GameSpeed::Quick);
        assert_eq!(session.params.leader_pool, LeaderPool::ExpandedHistorical);
        assert!(!session.game.victory_conditions.culture);
        assert!(!session.game.victory_conditions.score);
        drop(session);
        // The queue is spent: the world after this one is this one's settings.
        assert!(decorated_state(&shared)["next_game_settings"].is_null());
    }

    #[test]
    fn automatic_successor_seed_round_trips_exactly_through_javascript() {
        // This is the predecessor from the live failure. Its old full-width
        // successor was 7_959_629_191_918_103_844, which JavaScript rounded
        // before returning it in the painted-frame acknowledgement.
        let predecessor: u64 = 1_785_694_281;
        let old_full_width = predecessor
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        assert!(old_full_width > MAX_EXACT_JAVASCRIPT_INTEGER);

        let successor = automatic_successor_seed(predecessor);
        assert_eq!(successor, old_full_width & MAX_EXACT_JAVASCRIPT_INTEGER);
        assert!(successor <= MAX_EXACT_JAVASCRIPT_INTEGER);
        assert_eq!(successor as f64 as u64, successor);
    }

    /// The setup panel writes on every `change`, and `startNewSimulation`
    /// waits for that write before it asks for the restart — so a settings
    /// control that queued behind an AI turn put the restart behind it too.
    #[test]
    fn settings_are_staged_while_the_simulation_lock_is_held() {
        let mut params = current();
        params.spectate = true;
        let shared = shared_for(Session::new(params));

        let turn_in_flight = shared.session.lock().unwrap();
        let started = Instant::now();
        shared.stage_next_game_settings(&json!({"num_players": 6, "map_script": "continents"}));
        let staged = started.elapsed();

        assert_eq!(shared.staged_next_game_settings()["players"], json!(6));
        assert_eq!(shared.staged_next_game_settings()["map"], json!("continents"));
        assert!(
            staged < Duration::from_millis(50),
            "choosing a setting waited {staged:?} on the turn in flight"
        );
        drop(turn_in_flight);
    }

    /// A `Shared` around one session. The new-game request deliberately lives
    /// beside the simulation rather than inside it, so a test of it needs the
    /// same wrapper the server runs behind.
    fn shared_for(mut session: Session) -> Arc<Shared> {
        let seed = session.game.seed;
        Arc::new(Shared {
            current_seed: AtomicU64::new(seed),
            supervisor_request: Mutex::new(None),
            live_params: Mutex::new(session.params.clone()),
            next_game_params: Mutex::new(session.take_resumed_next_game_params()),
            session: Mutex::new(session),
            pace_ms: AtomicU64::new(0),
            between_game_countdown_ms: AtomicU64::new(DEFAULT_BETWEEN_GAME_COUNTDOWN_MS),
            paused: AtomicBool::new(false),
            restart_in: AtomicU64::new(u64::MAX),
            turn_us: AtomicU64::new(0),
            turn_compute_us: AtomicU64::new(0),
            frame_sequence: AtomicU64::new(0),
            frame_delivery: Mutex::new(FrameDelivery::default()),
            frame_painted: Condvar::new(),
            simulation_frame_gate: Mutex::new(()),
            latest: Mutex::new(None),
            turn_ready: Condvar::new(),
        })
    }

    /// What a reader of `/state` actually sees: the session's observation with
    /// everything `decorate` attaches from beside it.
    fn decorated_state(shared: &Shared) -> Value {
        let mut state = shared.session.lock().unwrap().state();
        super::decorate(&mut state, shared);
        state
    }

    #[test]
    fn supervised_new_game_request_normalizes_settings_without_replacing_the_live_game() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let session = Session::new(params);
        let original_seed = session.game.seed;
        let shared = shared_for(session);

        shared
            .request_supervised_new_game(&json!({
                "mode": "fresh_code",
                "paused": false,
                "replace_world": {
                    "server_instance": std::process::id(),
                    "seed": original_seed,
                },
                "num_players": 4,
                "map_script": "continents",
                "game_speed": "quick",
                "leader_pool": "historical",
                "victory_conditions": {"culture": false, "score": false},
            }))
            .unwrap();

        let state = decorated_state(&shared);
        assert_eq!(shared.session.lock().unwrap().game.seed, original_seed);
        // The outgoing world stops stepping, and reads as held rather than
        // stalled, without the request ever having touched the session.
        assert!(shared.paused.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(state["spectator_paused"], json!(true));
        // The same request is on the lock-free probe the supervisor polls.
        assert_eq!(
            shared.pending_new_game_request().expect("a pending request")["mode"],
            "fresh_code"
        );
        assert_eq!(state["supervisor_request"]["mode"], "fresh_code");
        assert_eq!(state["supervisor_request"]["paused"], false);
        assert_eq!(state["supervisor_request"]["source"], "unknown");
        assert_eq!(
            state["supervisor_request"]["server_instance"].as_u64(),
            Some(std::process::id() as u64)
        );
        assert_eq!(
            state["supervisor_request"]["settings"],
            json!({
                "seed": 1,
                "players": 4,
                "width": 60,
                "height": 38,
                "city_states": 6,
                "turns": 330,
                "base_ruleset": "civ6",
                "start_era": "ancient",
                "future_era": "classic",
                "turn_structure": "sequential",
                "map": "continents",
                "shape": "flat",
                "poles": "poles",
                "speed": "quick",
                "leader_pool": "historical",
                "teams": [],
                "victories": ["science", "religious", "diplomatic", "domination"],
            })
        );
    }

    /// The control that leaves a world must not queue behind that world.
    ///
    /// Pressing Restart took the simulation lock, so it waited out whatever AI
    /// turn was in flight. On the live exhibition that is not a small wait: a
    /// late turn on a 74x46 six-player world held it long enough that `/state`
    /// did not answer in 120 seconds and `/status` did not answer in 30, so
    /// the page gave up at its own fifteen, cleared the veil and flashed an
    /// error — and the supervisor, which read the request out of that same
    /// `/state`, never saw one. Holding the lock here is what a turn does.
    #[test]
    fn a_restart_is_accepted_while_the_simulation_lock_is_held() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let shared = shared_for(Session::new(params));
        let seed = shared
            .current_seed
            .load(std::sync::atomic::Ordering::Relaxed);

        let turn_in_flight = shared.session.lock().unwrap();
        let started = Instant::now();
        shared
            .request_supervised_new_game(&json!({
                "mode": "restart",
                "paused": false,
                "replace_world": {
                    "server_instance": std::process::id(),
                    "seed": seed,
                },
            }))
            .expect("a restart is accepted mid-turn");
        let accepted = started.elapsed();

        // And the supervisor can read it without the lock either.
        let pending = shared
            .pending_new_game_request()
            .expect("the supervisor's probe carries the request");
        assert_eq!(pending["mode"], "restart");
        assert!(
            accepted < Duration::from_millis(50),
            "the restart waited {accepted:?} on a turn it exists to abandon"
        );
        drop(turn_in_flight);
    }

    /// A browser can keep a result callback queued while the supervisor swaps
    /// the listening socket to a successor. The callback is allowed to retire
    /// only the process and seed it was created for, never the active world it
    /// happens to reach later on the same port.
    #[test]
    fn a_stale_supervised_restart_cannot_replace_the_successor_world() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let shared = shared_for(Session::new(params));
        let seed = shared
            .current_seed
            .load(std::sync::atomic::Ordering::Relaxed);
        let instance = std::process::id() as u64;

        for request in [
            json!({"mode": "restart", "paused": false}),
            json!({
                "mode": "restart",
                "paused": false,
                "replace_world": {"server_instance": instance, "seed": seed + 1},
            }),
            json!({
                "mode": "restart",
                "paused": false,
                "replace_world": {"server_instance": instance + 1, "seed": seed},
            }),
        ] {
            assert!(shared.request_supervised_new_game(&request).is_err());
        }

        assert!(shared.pending_new_game_request().is_none());
        assert!(!shared.paused.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(
            shared
                .current_seed
                .load(std::sync::atomic::Ordering::Relaxed),
            seed
        );
    }

    /// World identity prevents an old result page from replacing a successor,
    /// but it cannot prove that a callback created for the *current* world had
    /// any reason to end it. The browser labels its unattended finale timer so
    /// the server can require an actual terminal session for that one source.
    #[test]
    fn an_automatic_finale_cannot_restart_an_active_supervised_game() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let shared = shared_for(Session::new(params));
        let seed = shared
            .current_seed
            .load(std::sync::atomic::Ordering::Relaxed);
        let request = json!({
            "mode": "restart",
            "restart_source": "finale_countdown",
            "paused": false,
            "replace_world": {
                "server_instance": std::process::id(),
                "seed": seed,
            },
        });

        assert_eq!(
            shared
                .request_supervised_new_game(&request)
                .expect_err("an active exhibition is not a finale"),
            "automatic restart requires a finished game"
        );
        assert!(shared.pending_new_game_request().is_none());
        assert!(!shared.paused.load(std::sync::atomic::Ordering::Relaxed));

        shared.session.lock().unwrap().game.winner = Some(0);
        shared
            .request_supervised_new_game(&request)
            .expect("the same timer is valid once its exact game is finished");
        let pending = shared
            .pending_new_game_request()
            .expect("the finished game's restart is queued");
        assert_eq!(pending["source"], "finale_countdown");
        assert!(shared.paused.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn unsupervised_server_rejects_supervisor_new_game_requests() {
        let shared = shared_for(Session::new(current()));
        assert!(shared
            .request_supervised_new_game(&json!({"mode": "fresh_code"}))
            .is_err());
        assert!(shared.pending_new_game_request().is_none());
        assert!(decorated_state(&shared)["supervisor_request"].is_null());
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

        let shared = shared_for(session);
        assert!(shared
            .request_supervised_new_game(&json!({
                "mode": "restart",
                "paused": false,
                "replace_world": {
                    "server_instance": std::process::id(),
                    "seed": 8,
                },
            }))
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

        assert!(session.play_on(PlayOnMode::UntilNextVictory));
        assert_eq!(session.game.current, 0);
        assert!(session.game.winner.is_none());
        assert_eq!(session.game.max_turns, 40);
        assert_eq!(session.game.turn_limit(), None);
        let state = session.state();
        assert!(state["winner"].is_null());
        assert_eq!(state["decided"]["victory_type"], json!("science"));
        assert_eq!(state["decided"]["turn"], json!(12));
        assert_eq!(state["decided"]["mode"], json!("until_next_victory"));
        assert!(state["turn_limit"].is_null());
        // A live game has nothing to play on past, and says so rather than
        // quietly granting turns nobody won.
        assert!(!session.play_on(PlayOnMode::Indefinite));
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
        assert!(state["players"][0]["ai_name"]
            .as_str()
            .is_some_and(|name| name != "AI player"));
        session.step();
        let stepped = session.state();
        assert_eq!(stepped["players"][0]["ai_strategy"], "expansion");
        assert!(["Frontier", "Horizon", "Homestead", "Border"]
            .iter()
            .any(|prefix| stepped["players"][0]["ai_name"]
                .as_str()
                .is_some_and(|name| name.starts_with(prefix))));
        // The expanded HUD card reads the whole plan, not just its label.
        let plan = &stepped["players"][0]["ai_plan"];
        assert_eq!(plan["strategy"], "expansion");
        assert!(plan["desired_cities"].as_u64().is_some());
        assert!(plan["assessed_turn"].as_u64().is_some());
        assert!(plan["forces"].is_array());
    }

    #[test]
    fn unified_war_plan_crosses_the_json_and_browser_contract() {
        let session = Session::new(current());
        let plan = crate::ai::PlanReport {
            strategy: "conquest",
            victory_target: None,
            rush: false,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 37,
            peace_offers: Vec::new(),
            forces: Vec::new(),
            war: Some(crate::ai::WarPlanReport {
                enabled: true,
                selective: true,
                rapid: true,
                active: true,
                phase: Some("strike"),
                target_player: Some(1),
                objective_city: None,
                breakthrough_tech: Some(crate::name!("apprenticeship")),
                assault_unit: Some(crate::name!("man_at_arms")),
                predecessor: Some(crate::name!("swordsman")),
                breach_unit: Some(crate::name!("battering_ram")),
                required_bodies: 4,
                ready_bodies: 4,
                staged_bodies: 3,
                breach_ready: true,
                upgrade_gold_reserved: 123.46,
                appointed_turn: Some(21),
                appointments: 2,
                breakthroughs: 2,
                mobilizations: 1,
                declarations: 1,
                complete_package_declarations: 1,
                objectives_captured: 0,
                objectives_captured_within_ten: 0,
                appointment_to_tech_turns: 12,
                tech_to_declaration_turns: 4,
                declaration_to_capture_turns: 0,
                appointment_to_tech_samples: vec![12],
                tech_to_declaration_samples: vec![4],
                declaration_to_capture_samples: Vec::new(),
                aborts: std::collections::BTreeMap::from([("target no longer alive", 2)]),
            }),
        };

        let wire = session.plan_json(&plan);
        assert_eq!(wire["war"]["selective"], json!(true));
        assert_eq!(wire["war"]["rapid"], json!(true));
        assert_eq!(wire["war"]["phase"], json!("strike"));
        assert_eq!(wire["war"]["breakthrough_tech"], json!("apprenticeship"));
        assert_eq!(wire["war"]["ready_bodies"], json!(4));
        assert_eq!(wire["war"]["staged_bodies"], json!(3));
        assert_eq!(wire["war"]["upgrade_gold_reserved"], json!(123.5));
        assert_eq!(wire["war"]["aborts"]["target no longer alive"], json!(2));
        assert!(EMBEDDED_INDEX.contains("const warPlan = plan?.war || null;"));
        assert!(EMBEDDED_INDEX.contains("warPlan.selective"));
        assert!(EMBEDDED_INDEX.contains("warPlan.rapid"));
        assert!(EMBEDDED_INDEX.contains("row(\"Attack phase\""));
        assert!(EMBEDDED_INDEX.contains("row(\"Strike package\""));
    }

    /// The AI strategy dossier reads one civilization's plan beside what it
    /// is actually spending its science and culture on. The observation only
    /// ever carries the *observed* seat's study, in `me`, and above the world
    /// there is no observed seat — so the omniscient view names every major's.
    ///
    /// Only that view. Watching as one civilization means seeing what it can
    /// see, and a rival's laboratory is not on that list; the panel's picker
    /// collapses to the one seat precisely because the wire gives it one.
    #[test]
    fn only_the_omniscient_view_reads_a_rivals_laboratory() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        // A world at turn 0 has chosen nothing yet.
        for _ in 0..8 {
            session.step();
        }
        let omniscient = session.state();
        let majors = || {
            omniscient["players"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["is_minor"] == json!(false) && p["is_barbarian"] == json!(false))
        };
        assert!(majors().count() > 1, "a table needs rivals to withhold");
        assert!(
            majors().any(|p| p["research"].is_string()),
            "somebody has picked a technology by turn 8"
        );
        for player in majors() {
            for field in [
                "research",
                "research_progress",
                "research_boosted",
                "civic",
                "civic_progress",
                "civic_boosted",
            ] {
                assert!(
                    player.get(field).is_some(),
                    "the omniscient view owes {} its {field}",
                    player["civ"]
                );
            }
            assert!(player["research_boosted"].is_boolean());
            assert!(player["civic_boosted"].is_boolean());
            assert!(player["research_progress"].is_number());
        }
        // A city-state has no research panel to fill and is not annotated.
        for player in omniscient["players"].as_array().unwrap() {
            if player["is_minor"] == json!(true) || player["is_barbarian"] == json!(true) {
                assert!(player.get("research").is_none());
            }
        }

        session.set_view_player(Some(1)).unwrap();
        let watched = session.state();
        for player in watched["players"].as_array().unwrap() {
            assert!(
                player.get("research").is_none() && player.get("civic").is_none(),
                "watching as one civilization must not read {}'s laboratory",
                player["civ"]
            );
        }
        // The watched seat still reads its own out of `me`, as it always has.
        assert!(watched["me"].get("research").is_some());
        assert!(watched["me"]["boosted_techs"].is_array());
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
    fn selecting_all_players_promotes_the_live_match_to_omniscient_spectator_mode() {
        let mut session = Session::new(current());
        assert!(!session.params.spectate);

        session.set_view_player(None).unwrap();
        let spectator_view = session.state();

        assert!(session.params.spectate);
        assert_eq!(spectator_view["spectate"].as_bool(), Some(true));
        assert!(spectator_view["view_player"].is_null());
        assert_eq!(
            spectator_view["visible"].as_array().unwrap().len(),
            session.game.map.tiles.len()
        );
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
            .insert(crate::name!("campus"), district_pos);
        game.cities
            .get_mut(&first_city)
            .unwrap()
            .wonders
            .insert(crate::name!("pyramids"), district_pos);
        game.cities
            .get_mut(&first_city)
            .unwrap()
            .buildings
            .push(crate::name!("granary"));
        game.cities.get_mut(&first_city).unwrap().pop = 4;
        game.players[0].religion = Some("Test Faith".to_string());
        game.players[0].government = Some("classical_republic".to_string());
        game.players[0].techs.insert(crate::name!("horseback_riding"));
        game.players[0].civics.insert(crate::name!("drama_poetry"));
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
                unit.owner == 1 && game.rules.units[unit.kind].class == "military"
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
            "id=\"mapseed\"",
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
        // The leader picker follows the live roster API, while difficulty is
        // still supplied by the active ruleset; neither is a hardcoded list.
        assert!(EMBEDDED_INDEX.contains("RULES.leader_pools"));
        assert!(EMBEDDED_INDEX
            .contains("RULES.difficulties && typeof RULES.difficulties === \"object\""));
        // A spectated world has nobody to hand a leader or a handicap to, and
        // neither has a Civilization VI run — its seat's civilization is dealt
        // by that game. Its *difficulty* still travels, because that is the one
        // setting the mode exists for.
        assert!(EMBEDDED_INDEX.contains(
            "...(gameMode === \"ai_sim\"\n            ? (leaderSelection === \"custom\" ? {civs: customCivs} : {})"
        ));
        assert!(EMBEDDED_INDEX.contains(
            ": {civs: civ6 || !leader ? [] : [leader], difficulty})"
        ));
        // The seed is a common world setting, while the leader and difficulty
        // above remain the single-player additions. City-states follow the
        // selected map-size profile, and the turn cap follows the selected
        // game speed; neither needs a separate control.
        assert!(EMBEDDED_INDEX.contains("setOptionalWorldNumber(\"mapseed\", st.seed);"));
        // A build without the save endpoints hides the group rather than
        // offering one that cannot work.
        assert!(EMBEDDED_INDEX.contains("catch (error) { group.style.display = \"none\";"));
    }

    /// Choosing single player and pressing the one start control on screen
    /// must open that game — on the supervised exhibition too, where every
    /// simulation is a fresh process but a human game takes this one over.
    /// The control itself persists when the world changes; only its presentation
    /// transforms, so a second Start new game button never materializes.
    #[test]
    fn browser_transforms_restart_control_for_single_player() {
        assert!(EMBEDDED_INDEX
            .contains("const supervised = !!(state && state.supervised) && payload.spectate;"));
        // Only a world that is definitely spectated leaves this a restart.
        assert!(EMBEDDED_INDEX.contains("const human = settings.spectate !== true;"));
        // Choosing single player renames that control after the game it opens,
        // rather than leaving "Restart sim" over a single-player subtitle.
        assert!(EMBEDDED_INDEX.contains("<span class=\"lbl\">Restart sim</span>"));
        assert!(EMBEDDED_INDEX.contains("button.classList.toggle(\"human-start\", human);"));
        assert!(EMBEDDED_INDEX.contains("? \"Start new game\""));
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
        assert!(EMBEDDED_INDEX.contains(
            "document.getElementById(\"specbar\").style.display = \"block\";"
        ));
        // Adopting the running mode still re-reads the panel and relabels the
        // control, so the world that just arrived is described by the control
        // that will replace it.
        assert!(EMBEDDED_INDEX.contains(
            "syncSetupMode();\n  updateRestartSimulationButton();"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "body:not(.watching-sim) .spec-controls:has(#restart-sim) {"
        ));
        assert_eq!(EMBEDDED_INDEX.matches("id=\"restart-sim\"").count(), 1);
        assert!(!EMBEDDED_INDEX.contains("id=\"startgame\""));
        assert!(EMBEDDED_INDEX.contains("document.body.classList.toggle(\"watching-sim\", SPEC);"));
        // Leader and difficulty still do follow the selection. The leader row
        // carries a second class because a third mode hides it for a different
        // reason — see `browser_keeps_the_civilization_vi_mode_available_for_verification`.
        assert!(EMBEDDED_INDEX.contains("body.spectating .human-setting { display: none; }"));
        assert!(EMBEDDED_INDEX.contains(
            "class=\"small game-advanced-setting human-setting civ6-hidden\""
        ));
        assert!(EMBEDDED_INDEX.contains(
            "class=\"small game-advanced-setting human-setting\""
        ));
        // Settings staged for the next simulation describe a spectated world,
        // so they may only adopt that mode while one is on screen.
        assert!(EMBEDDED_INDEX
            .contains("if (SPEC) document.getElementById(\"gamemode\").value = \"ai_sim\";"));
    }

    /// The verification-only mode plays the other game, so the panel it is
    /// chosen in has to stop describing one of ours.
    ///
    /// Every assertion here is a setting that would otherwise stay on screen
    /// and be silently dropped. `CivvisControlSetup.lua` writes ruleset, map,
    /// size, difficulty and speed into Civilization VI and nothing else, so a
    /// leader, a team, a start era or a victory condition offered in this mode
    /// is a promise about the run that the run does not keep — and the run
    /// takes hours to disprove it.
    #[test]
    fn browser_keeps_the_civilization_vi_mode_available_for_verification() {
        // This route remains selectable by verification, but it is not a
        // public choice in the left-panel game-mode list.
        assert!(EMBEDDED_INDEX.contains(
            "<option value=\"civ6\" hidden>Play Firaxis Civ 6 with computer control</option>"
        ));
        // A third body state, not a variation on either of the other two.
        assert!(EMBEDDED_INDEX
            .contains("document.body.classList.toggle(\"playing-civ6\", civ6);"));
        assert!(EMBEDDED_INDEX.contains("body.playing-civ6 .civ6-hidden { display: none; }"));
        // Exactly the rows that are not carried, and no others. Difficulty is
        // deliberately absent: it is the setting the mode exists for.
        for row in [
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"20\"", // teams
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"30\"", // leader pool
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"40\"", // leader selection
            "class=\"custom-leader-selection game-advanced-setting civ6-hidden\"", // custom table
            "class=\"small game-advanced-setting human-setting civ6-hidden\"",
            "class=\"small civ6-hidden\">World shape",
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"50\"", // thermal
            "class=\"small game-advanced-setting civ6-hidden\" data-advanced-order=\"60\"", // map seed
            "class=\"small civ6-hidden\">Start era",
            "class=\"victory-options civ6-hidden\"",
            "class=\"advanced-settings civ6-hidden\" id=\"game-mod-settings\"",
        ] {
            assert!(EMBEDDED_INDEX.contains(row), "{row} is not hidden in the Civ 6 mode");
        }
        assert!(!EMBEDDED_INDEX.contains("class=\"small human-setting civ6-hidden\">Difficulty"));
        // The map control becomes the other game's roster rather than a
        // filtered copy of ours; neither list contains the other.
        assert!(EMBEDDED_INDEX.contains("function syncMapRoster(civ6)"));
        assert!(EMBEDDED_INDEX.contains("const carried = maps.find(map => map.civvis === chosen);"));
        // The one start control is named after the game it starts.
        assert!(EMBEDDED_INDEX.contains("? \"Play Firaxis Civ 6\""));
        assert!(EMBEDDED_INDEX
            .contains("if (readSetting(\"gamemode\") === \"civ6\") { startCiv6Game(); return; }"));
        assert!(EMBEDDED_INDEX.contains("await fetchJSON(\"/civ6/start\", {method: \"POST\","));
        // A refusal is shown rather than hidden. A run that silently never
        // starts is how a dead Steam client cost eleven ladder attempts.
        assert!(EMBEDDED_INDEX.contains("civ6Status = await fetchJSON(\"/civ6\");"));
        assert!(EMBEDDED_INDEX.contains("`Cannot start: ${status.blocked}`"));
        assert!(EMBEDDED_INDEX.contains("button.disabled = blocked || !!setupError;"));
    }

    /// The verification-only mode is available on every computer and refused
    /// on the ones that cannot run it, so both of its endpoints always answer
    /// — and the refusal is a sentence a person can act on rather than a
    /// missing response.
    #[test]
    fn the_civ6_endpoints_answer_on_any_host() {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("a free port")
            .local_addr()
            .unwrap()
            .port();
        let mut params = current();
        params.spectate = true;
        params.num_players = 2;
        params.num_city_states = 0;
        params.width = 24;
        params.height = 16;
        params.seed = 20_260_731;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, true));
        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "the server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        let json_at = |target: &str| -> Value {
            serde_json::from_str(&http_get(port, target).expect(target)).expect(target)
        };

        let host = json_at("/civ6");
        assert!(host["ready"].is_boolean(), "{host}");
        assert_eq!(host["ready"].as_bool(), Some(host["blocked"].is_null()));
        if let Some(blocked) = host["blocked"].as_str() {
            assert!(!blocked.is_empty() && !blocked.contains('\n'), "{blocked:?}");
        }
        // The other game's vocabulary rides on the ruleset, because it never
        // changes while a server runs — unlike the host report above, which is
        // a question about this machine's installation.
        let rules = json_at("/rules");
        let civ6 = &rules["civ6"];
        assert_eq!(civ6["maps"].as_array().map(Vec::len), Some(crate::civ6::MAPS.len()));
        assert_eq!(civ6["difficulties"].as_array().map(Vec::len), Some(8));
        assert_eq!(civ6["default_map"].as_str(), Some(crate::civ6::DEFAULT_MAP));
        // Every map names a script this build would pass to the other game.
        for map in civ6["maps"].as_array().unwrap() {
            assert!(map["id"].as_str().is_some_and(|id| id.ends_with(".lua")), "{map}");
        }
        // A start is refused, with a reason, rather than 404ing or hanging —
        // which is the only claim this test can make about starting one,
        // because the other one takes over the computer for hours.
        let refused: Value = serde_json::from_str(
            &http_post(port, "/civ6/start", &json!({"difficulty": "not-a-rung"}).to_string())
                .expect("a refusal"),
        )
        .expect("refusal JSON");
        assert_eq!(
            refused["error"].as_str(),
            Some("no Civilization VI difficulty is called \"not-a-rung\"")
        );
        assert!(refused["started"].is_null());
    }

    /// Diplomacy needs a complete player-facing action surface. The screen
    /// must retain both war/peace (including city-states) and every major-to-
    /// major relationship action, or a mechanic the engine and AI can use is
    /// still unavailable to a human player.
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
            "send_delegation",
            "send_embassy",
            "propose_defensive_pact",
            "propose_joint_war",
            "request_promise",
            "demand_gold",
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

    /// Secondary-clicking a distant tile is Civ 6's "go there", and it cannot
    /// be built on `move_to`: `path_to` seeds its search with the unit's
    /// remaining movement, so anything further is `"unreachable"`. `/route`
    /// exposes the long-range router the AI already uses, one step at a time,
    /// and the client still sends a normal Move for that step — so the engine
    /// stays the authority on whether the move is legal now.
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

        if let Some(step) = session.game.route_step(unit, far, 0) {
            assert_ne!(step, start, "a route step must leave where it started");
            assert_eq!(
                session.game.wdist(start, step),
                1,
                "a route step is one tile, validated by the caller's Move"
            );
        }
        // An island start can legitimately have no land route; the client treats that
        // the same way — the order ends rather than retrying.

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
        // Both finales also offer three ways to keep this world. It is the
        // reason the countdown has to be long enough to read — a button nobody
        // can reach before the next world loads is not an offer.
        assert!(EMBEDDED_INDEX.contains("id=\"play-on-look-around\""));
        assert!(EMBEDDED_INDEX.contains("id=\"play-on-next-victory\""));
        assert!(EMBEDDED_INDEX.contains("id=\"play-on-indefinite\""));
        assert!(EMBEDDED_INDEX.contains("Take a look around"));
        // The two rules that resume play are named for what the person wants
        // rather than for the rule they select. "Continue" alone did not say
        // what it continues *to* — the two play-on buttons differ only in
        // which later result stops them, and a bare verb left that on the
        // tooltip where nobody reads it.
        assert!(EMBEDDED_INDEX.contains(">Continue to the next Victory type</button>"));
        assert!(EMBEDDED_INDEX.contains(">To infinity and beyond</button>"));
        assert!(EMBEDDED_INDEX.contains(
            "title=\"Keep playing this world without a turn limit. The exact result shown \
             here will not repeat; the next distinct victory ends the game.\">\
             Continue to the next Victory type<"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "title=\"Keep playing this world without a turn limit and ignore every later \
             victory.\">To infinity and beyond<"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "playOnPastVictory('until_next_victory', true)"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "playOnPastVictory('until_next_victory', false)"
        ));
        assert!(EMBEDDED_INDEX.contains("playOnPastVictory('indefinite', false)"));
        assert!(EMBEDDED_INDEX.contains("async function playOnPastVictory(mode, paused)"));
        assert!(EMBEDDED_INDEX.contains("body: JSON.stringify({mode, paused})"));
        assert!(EMBEDDED_INDEX.contains("cancelSupervisedSuccessorWatch();"));
    }

    /// A spectated finale is counted down by the supervisor. A finished human
    /// game has nobody driving it, so its own result screen counts down to the
    /// next game — and every way of saying "I am still here" has to stop it,
    /// or the offer to keep the world is only an offer for the selected hold.
    #[test]
    fn a_human_finale_counts_itself_down_to_the_next_game() {
        assert!(EMBEDDED_INDEX.contains(
            "finaleCountdownDeadline = Date.now() + betweenGameCountdownMs();"
        ));
        assert!(EMBEDDED_INDEX.contains("id=\"finale-restart\""));
        assert!(EMBEDDED_INDEX
            .contains("button.textContent = `${FINALE_RESTART_LABEL} (${left})`;"));
        // The supervisor owns the exhibition's handoff, so a spectated finale
        // never arms this one on top of the countdown it already publishes.
        assert!(EMBEDDED_INDEX
            .contains("if (SPEC || finaleCountdownResult === signature) return;"));
        // Both human endings count down: a victory and a last city lost.
        assert_eq!(EMBEDDED_INDEX.matches("armFinaleCountdown(signature);").count(), 2);
        // Any input stops it, the three ways to keep the world stop it, and a
        // result screen that goes away takes it with it.
        assert!(EMBEDDED_INDEX.contains(
            "for (const gesture of [\"pointerdown\", \"keydown\", \"wheel\"])"
        ));
        assert!(EMBEDDED_INDEX.contains("cancelFinaleCountdown(),\n    {capture: true, passive: true});"));
        assert!(EMBEDDED_INDEX.contains("cancelSupervisedSuccessorWatch();\n  cancelFinaleCountdown();"));
        assert!(EMBEDDED_INDEX.contains("clearFinaleCountdown();"));
        // Reaching zero starts the same flow as the button, but identifies the
        // unattended caller so the server can require a genuinely finished
        // session before it accepts a supervised handoff.
        assert!(EMBEDDED_INDEX.contains(
            "cancelFinaleCountdown();\n  // Name the only non-human caller."
        ));
        assert!(EMBEDDED_INDEX.contains("startNewSimulation(\"finale_countdown\");"));
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
            !names.contains(&"strategic"),
            "a league-only search entrant is not a live auto-play offer"
        );
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
        assert_eq!(
            session.seat_strategy_at(0, "strategic"),
            Err("no strategy named strategic".to_string()),
            "a direct request must not bypass the live-eligibility boundary"
        );
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

    #[test]
    fn an_explicit_coverage_target_can_include_a_live_excluded_strategy() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-server-coverage-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let dir = dir.to_str().unwrap().to_string();
        let _ = std::fs::remove_dir_all(&dir);
        let entrant = |name: &str| {
            crate::league::Strategy::new(
                name,
                crate::league::StrategyKind::Builtin {
                    ai: name.to_string(),
                },
                0,
            )
        };
        let mut excluded = entrant("strategic");
        assert!(excluded.exclude_from_live("coverage test", "the test ends"));
        crate::league::save_league(
            &dir,
            &crate::league::League {
                round: 2,
                strategies: vec![entrant("advanced"), entrant("basic"), excluded],
                calibration: Default::default(),
            },
        );

        let mut params = current();
        params.num_players = 4;
        params.spectate = true;
        params.league_dir = Some(dir.clone());
        params.force_strategy = Some("strategic".to_string());
        let session = Session::new(params);
        let target = session
            .league
            .as_ref()
            .unwrap()
            .strategies
            .iter()
            .position(|strategy| strategy.name == "strategic")
            .unwrap();
        assert_eq!(session.seat_strategy[0], Some(target));
        assert_eq!(session.seat_entry(0).unwrap().name, "strategic");
        assert_eq!(
            session.state()["players"][0]["ai_player_strategy"],
            json!("strategic"),
            "terminal evidence carries the stable league identity"
        );
        std::fs::remove_dir_all(&dir).unwrap();
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

    /// Every major on screen carries a rating, whether or not this game is
    /// being rated into anything.
    ///
    /// The elo column used to be empty for every seat in every game that was
    /// not launched with `--league`, which is almost all of them — a table of
    /// dashes that reads as "this build cannot rate anyone". Two things fix
    /// it: the snapshot every build ships is loaded for labels, and a seat
    /// the league has no row for shows the provisional base rather than
    /// nothing.
    #[test]
    fn every_major_carries_a_rating_without_a_named_roster() {
        let mut params = current();
        params.spectate = true;
        params.num_players = 4;
        let session = Session::new(params);
        assert!(
            session.league.is_some(),
            "the shipped snapshot labels a game that names no roster"
        );
        assert!(!session.seat_from_roster, "and it seats nobody");

        let state = session.state();
        let majors: Vec<&Value> = state["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["is_minor"] != json!(true) && p["is_barbarian"] != json!(true))
            .collect();
        assert_eq!(majors.len(), 4);
        let mut shares = 0.0;
        let mut now_shares = 0.0;
        for player in &majors {
            let elo = player["ai_elo"].as_i64().expect("every major has a rating");
            assert!((800..=2600).contains(&elo), "{elo} is not a rating");
            assert!(player["ai_elo_rd"].as_i64().is_some());
            shares += player["odds_start"].as_f64().expect("and start odds");
            now_shares += player["odds_now"].as_f64().expect("and now odds");
            assert!(player["odds_prior_elo"].as_i64().is_some());
            // Nobody chose these seats, so the one entrant behind all of them
            // does not get to put its handle on four different civilizations.
            assert!(
                player["ai_username"].is_null(),
                "an unseated table keeps its per-seat names"
            );
            assert!(player["ai_name"].as_str().is_some_and(|n| !n.is_empty()));
        }
        assert!(
            (shares - 1.0).abs() < 0.02,
            "one table, one winner: start odds summed to {shares}"
        );
        assert!(
            (now_shares - 1.0).abs() < 0.02,
            "and so does the live answer: now odds summed to {now_shares}"
        );
        // An earned rating, not the base everybody starts from. The check
        // above passes on a provisional 1500 too, which is exactly what the
        // whole table showed while the roster was read from a relative path.
        for player in &majors {
            assert_eq!(
                player["ai_elo_provisional"],
                json!(false),
                "the shipped roster has games behind it"
            );
        }
    }

    /// A seated player is told their own odds, and an unmet rival's are withheld
    /// along with everything else about them.
    ///
    /// This is the fog rule the whole annotation lives under, and the seat it
    /// matters most for is the viewer's own: somebody playing the game wants to
    /// know what they were given and where they stand, and `has_met` answers
    /// true for yourself, so their row is annotated from turn one.
    #[test]
    fn a_seated_player_sees_their_own_odds_and_not_an_unmet_rivals() {
        let mut params = current();
        params.spectate = false;
        params.num_players = 4;
        let session = Session::new(params);
        let state = session.state();
        let me = state["player"].as_u64().expect("an interactive game has a seat");
        let mut unmet = 0;
        for player in state["players"].as_array().expect("a player list") {
            let is_major = player["is_minor"] != json!(true) && player["is_barbarian"] != json!(true);
            if !is_major {
                continue;
            }
            if player["id"] == json!(me) {
                assert!(
                    player["odds_start"].as_f64().is_some_and(|odds| odds > 0.0),
                    "your own seat carries its start odds: {player}"
                );
                assert!(player["odds_now"].as_f64().is_some());
                continue;
            }
            if player["met"] == json!(false) {
                unmet += 1;
                assert!(
                    player["odds_start"].is_null() && player["odds_now"].is_null(),
                    "an unmet rival is not annotated at all: {player}"
                );
            }
        }
        assert!(unmet > 0, "a fresh interactive game has civilizations still to meet");
    }

    /// The roster that labels an ordinary game is compiled in, so the ratings
    /// on screen are the same wherever the binary was started from.
    ///
    /// The label roster ended at a read of `data/league`, a directory resolved
    /// against the working directory. `cargo test` runs at the crate root and
    /// a developer is usually standing in a checkout, so the read succeeded
    /// exactly where anyone would look and failed everywhere the program
    /// actually ships: the installed binary run from a home directory, a
    /// launcher that starts it from `/`, and the WASM build, which has no
    /// filesystem at all. Every seat in those games showed a provisional 1500.
    #[test]
    fn the_label_roster_is_compiled_in_rather_than_read_from_the_working_directory() {
        let shipped = crate::league::shipped_league().expect("the snapshot is compiled in");
        let mut params = current();
        params.league_dir = None;
        let (league, seats) = Session::load_params_league(&params);
        let league = league.expect("a game that names no roster is still labelled");
        assert!(!seats, "a label roster seats nobody and rates nothing");
        // A checkout that has been recording locally answers from its own
        // runtime `league/` first, which is the point of that step. With no
        // such directory — a fresh checkout, and every shipped build — the
        // chain has to reach the compiled-in snapshot rather than stop at a
        // path that is not there.
        if crate::league::load_league("league").is_none() {
            assert_eq!(
                league.strategies.len(),
                shipped.strategies.len(),
                "the labels come from the shipped roster"
            );
        }
    }

    /// An interactive game rates its rivals on screen too — but only the
    /// ones you have met, and never their plan.
    ///
    /// Only the spectator used to annotate ratings, so the HUD's ELO column
    /// was empty for every opponent in every game somebody was actually
    /// playing.
    #[test]
    fn an_interactive_game_rates_the_rivals_you_have_met() {
        let mut params = current();
        params.num_players = 3;
        let session = Session::new(params);
        let state = session.state();
        assert_eq!(state["spectate"], json!(false));
        let players = state["players"].as_array().unwrap();
        let mut met = 0;
        for player in players {
            let unmet = player["met"] == json!(false);
            let minor = player["is_minor"] == json!(true) || player["is_barbarian"] == json!(true);
            if unmet || minor {
                assert!(
                    player["ai_elo"].is_null(),
                    "an unmet or minor seat is not annotated"
                );
                continue;
            }
            met += 1;
            assert!(player["ai_elo"].as_i64().is_some(), "a met major is rated");
            // Their standing is public; what they intend to do with it is not.
            assert!(player["ai_plan"].is_null());
            assert!(player["ai_strategy"].is_null());
        }
        assert!(met > 0, "the player's own seat is met at the very least");
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
        // Nothing is being rated, and they have still never finished a game,
        // so the seat wears the rating everybody starts from and says so.
        assert_eq!(state["players"][0]["player_elo"], json!(1500));
        assert_eq!(state["players"][0]["player_elo_rd"], json!(350));
        assert_eq!(state["players"][0]["player_elo_provisional"], json!(true));
        assert_eq!(state["players"][0]["player_games"], json!(0));

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

    /// A single-player page has to be able to read its own first state.
    ///
    /// The opening `/state?painted=` of every page takes
    /// `simulation_frame_gate` — that is how a page attaching mid-turn is
    /// ordered against the stepper. The stepper does not step a game somebody
    /// is playing, but it used to take that gate *first* and only then notice,
    /// sleeping 300ms with the gate held and retaking it the moment it let go.
    /// The window left for the page was too narrow to win, so a browser opening
    /// a single-player game was starved out of its first snapshot: boot aborts
    /// at fifteen seconds, retries, and is starved again. The map never
    /// appears, and auto-play has nothing to draw a frame from.
    #[test]
    fn a_single_player_page_gets_its_first_state_at_once() {
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
        assert!(!params.spectate, "this test is about the human-game path");
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "single-player server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        // Exactly what boot asks for: a viewer that has painted nothing yet.
        // Asked repeatedly, because starvation is a race and one lucky read
        // proves nothing.
        for attempt in 1..=5 {
            let started = Instant::now();
            let body = http_get(port, "/state?painted=&viewer=boot-probe")
                .unwrap_or_else(|| panic!("read {attempt}: the opening state never arrived"));
            let elapsed = started.elapsed();
            let state: Value = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("read {attempt}: opening state is not JSON: {e}"));
            assert!(
                state["turn"].is_number(),
                "read {attempt}: the opening state carries no turn"
            );
            assert!(
                elapsed < Duration::from_secs(5),
                "read {attempt}: the opening state took {elapsed:?}, and a page gives up at 15s"
            );
        }
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
        // The choice rides on every request, so a run continued turn by turn
        // cannot change agent halfway through.
        assert!(EMBEDDED_INDEX.contains("async function autoplayBatch(turns, strategy)"));
        assert!(EMBEDDED_INDEX.contains("request_id: requestId"));
        assert!(EMBEDDED_INDEX.contains("AUTOPLAY_BATCH_TIMEOUT_MS = 120000"));
        // Pressing it again stops, rather than queueing a second run.
        assert!(EMBEDDED_INDEX.contains("if (autoplaying) { autoplayStop = true; return; }"));
        assert!(EMBEDDED_INDEX.contains("while (inFlight)"));
    }

    /// Every turn auto-play simulates has to reach the screen as a frame.
    ///
    /// This is Martin's standing requirement, and auto-play was the one loop
    /// that broke it. It asked the engine for a *batch* of turns and got back
    /// only the state after the last one, so a batch of ten played ten turns
    /// and drew one — nine turns simulated, delivered to nobody, gone. The
    /// batch doubled while responses were quick, which is why it looked like
    /// turns were skipped "sometimes": the run began at one turn per request
    /// and grew away from it within a second.
    ///
    /// Two properties fix it and both are load-bearing. The request is for one
    /// turn, so every turn has a state of its own. And the paint happens inside
    /// an animation frame, because two states rendered inside one display
    /// refresh are composited into a single frame — a turn can otherwise be
    /// simulated, delivered, drawn, and still never appear on a screen.
    #[test]
    fn auto_play_draws_a_frame_for_every_turn_it_plays() {
        // One turn per request, both the first and every one after it.
        assert!(EMBEDDED_INDEX.contains("inFlight = autoplayBatch(1, strategy);"));
        assert!(EMBEDDED_INDEX.contains(
            "inFlight = played > 0 && left > 0 && !autoplayStop ? autoplayBatch(1, strategy) : null;"
        ));
        // One presented frame per turn.
        assert!(EMBEDDED_INDEX.contains(
            "return new Promise(drawn => requestAnimationFrame(() => { render(st); drawn(); }));"
        ));
        assert!(EMBEDDED_INDEX.contains("await autoplayFrame(next);"));
        // The adaptive batch is what dropped the turns. It must not come back,
        // in any of the three shapes it had.
        for gone in [
            "batch = elapsed < 250",
            "const ask = Math.min(batch, left)",
            "await autoplayBatch(ask, strategy)",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(gone),
                "auto-play is batching turns again, so turns go unseen: {gone}"
            );
        }
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

    #[test]
    fn browser_next_action_prefers_nearby_unvisited_units_before_revisiting() {
        let start = EMBEDDED_INDEX
            .find("const nextActionVisited = new Set();")
            .expect("next action should retain a visited-unit pass");
        let end = EMBEDDED_INDEX[start..]
            .find("// Civ 6's explicit previous/next-unit controls")
            .map(|offset| start + offset)
            .expect("the ordinary unit cycle should remain separate from next action");
        let action_pass = &EMBEDDED_INDEX[start..end];

        // Mark the current unit before looking for candidates, so the action
        // moves onward instead of selecting the unit already under review.
        let mark_current = action_pass
            .find("if (origin) nextActionVisited.add(origin.id);")
            .expect("next action should mark the current unit visited");
        let fresh_candidates = action_pass
            .find("let candidates = waiting.filter(unit => !nextActionVisited.has(unit.id));")
            .expect("next action should prefer unvisited units");
        assert!(
            mark_current < fresh_candidates,
            "the current unit must be visited before the fresh candidate set is made"
        );
        let revisit = action_pass
            .find("if (!candidates.length) {")
            .expect("next action should reopen visited units after a full pass");
        assert!(
            fresh_candidates < revisit,
            "visited units may be reconsidered only after every fresh candidate"
        );
        assert!(action_pass.contains("nextActionVisited.clear();"));
        assert!(action_pass.contains("candidates = waiting.filter(unit => !origin || unit.id !== origin.id);"));
        // `whexDist` keeps the nearest-unit promise true across wrapped map
        // seams instead of measuring the long way around the world.
        assert!(action_pass.contains(
            "whexDist(origin.pos, first.pos) - whexDist(origin.pos, second.pos)"
        ));
        assert!(action_pass.contains("nextActionVisited.add(sel.id);"));
        assert!(EMBEDDED_INDEX.contains(
            "function nextAction() {\n  if (!state || SPEC) return;\n  advanceToNextActionUnit(true);"
        ));
        // The requested fixed action map invokes the new selector from key 1;
        // unit roster cycling is no longer a global keyboard shortcut.
        assert!(EMBEDDED_INDEX.contains("if (step > 0) { advanceToNextUnit(true); return; }"));
        assert!(EMBEDDED_INDEX.contains(
            "{id: \"NextAction\", key: \"1\", run: () => nextAction()},"
        ));
        assert!(!EMBEDDED_INDEX.contains("id: \"NextUnitTab\""));
    }

    /// Resting over a tile reports it, the way Civ 6's plot tooltip does — and
    /// it keeps doing so after the map has been panned or the simulation advances.
    ///
    /// `dragMoved` outlives its gesture: the click that follows clears it, and
    /// a drag released off the canvas never produces one. A hover guard that
    /// reads it therefore goes permanently quiet after the first pan, which is
    /// exactly how the tooltip died. The guard has to ask whether a gesture is
    /// in flight *now*.
    #[test]
    fn resting_over_a_tile_delays_details_survives_a_pan_and_tracks_new_turns() {
        assert!(
            EMBEDDED_INDEX.contains(
                "dragState || mapTouches.size || rdrag) {"
            ),
            "the hover guard must test a live gesture, never the stale dragMoved flag"
        );
        assert!(
            EMBEDDED_INDEX.contains("const TILE_TIP_DELAY_MS = 350;")
                && EMBEDDED_INDEX.contains("(!reveal && !tileTipVisible)")
                && EMBEDDED_INDEX.contains("}, TILE_TIP_DELAY_MS);"),
            "tile details must wait for the pointer to rest, even across snapshot renders"
        );
        assert!(
            EMBEDDED_INDEX.contains("drawCaptureChoice();\n  refreshTileTip();"),
            "each delivered simulation snapshot must refresh an open tile tooltip"
        );
        for piece in [
            "function tileMoveCost(t)",
            "function tileDefense(t)",
            "function appealBand(appeal)",
            "function tileYieldMarkers(yields)",
            "function tileDetailYieldWords(yields, sign = false)",
            "const TILE_TIP_YIELD_ORDER = [\"food\", \"production\", \"gold\", \"science\", \"culture\", \"faith\"];",
            "class=\"tip-yield-marker\"",
            "style=\"--tip-yield-fill:${YPIP[kind]};--tip-yield-ink:${YINK[kind]}\"",
            "function tileBuiltTipLines(t, city)",
            "function districtTipLines(t)",
            "Base district yields:",
            "adjacencyLines(t.adjacency, \"Adjacency yields\", t.district)",
            "Total district yields:",
            "function tileResourceTipLine(t)",
            "★ ${cityName} ★",
            " · <span class=\"tip-yields\">${yieldMarkers}</span>",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the tile tooltip is missing {piece}"
            );
        }
        let tip_start = EMBEDDED_INDEX
            .find("function tileTipLines(t, pos, tileKey) {")
            .expect("the map detail formatter is declared");
        let tip_end = EMBEDDED_INDEX[tip_start..]
            .find("\n// tooltip")
            .map(|offset| tip_start + offset)
            .expect("the map detail formatter ends before tooltip lifecycle code");
        let details = &EMBEDDED_INDEX[tip_start..tip_end];

        // These are intentionally a stable reading order: unit, the thing built
        // on the tile and its yield ledger, owning empire/city, the ground and
        // its total yields, movement, defense, then appeal.
        let ordered = [
            "lines.push(`<span class=\"tip-unit\">${civPossessive(civ)} ${titleCase(unit.type)} - ",
            "lines.push(...tileBuiltTipLines(t, city));",
            "const owner = tileOwnershipTipLine(t);",
            "lines.push(tileTerrainTipLine(t));",
            "const resource = tileResourceTipLine(t);",
            "lines.push(...tileBaseYieldLines(t));",
            "const yields = tileTotalYields(t);",
            "lines.push(`Movement: ${movement}`);",
            "lines.push(`Defense: ${defenseText}`);",
            "lines.push(\"Appeal: \" + (t.appeal > 0 ? \"+\" : \"\") + t.appeal +",
        ];
        let mut previous = 0;
        for piece in ordered {
            let at = details
                .find(piece)
                .unwrap_or_else(|| panic!("the map-detail order is missing {piece}"));
            assert!(
                at >= previous,
                "{piece} must follow the preceding map-detail row"
            );
            previous = at;
        }
        assert!(details.contains("${civPossessive(civ)} ${titleCase(unit.type)} - "));
        assert!(details.contains(
            "const unitStatus = unitHasHealth(unit) ? `${fmtYield(unit.hp)} HP` : \"capturable\";"
        ));
        assert!(details.contains("unitStatus + \"</span>\""));
        assert!(details.contains("? (tileImpassable(t) ? \"Impassable\" : \"Unknown\")"));
        assert!(details.contains("const defenseText = (defense > 0 ? \"+\" : \"\") + defense;"));
        assert!(!details.contains(" MP"));
        assert!(!details.contains("Capital: "));
        assert!(!details.contains("<b>"));
        assert!(
            !details.contains("districtLensLabel("),
            "supplemental lens details use words rather than the old emoji-yield helper"
        );
        let terrain_start = EMBEDDED_INDEX
            .find("function tileTerrainTipLine(t) {")
            .expect("the terrain detail formatter is declared");
        let terrain_end = EMBEDDED_INDEX[terrain_start..]
            .find("\nfunction tileResourceTipLine")
            .map(|offset| terrain_start + offset)
            .expect("the terrain detail formatter ends before resources");
        let terrain = &EMBEDDED_INDEX[terrain_start..terrain_end];
        let feature = terrain.find("if (t.feature)").expect("features are included first");
        let ground = terrain.find("geography.push(titleCase(t.terrain)").expect("terrain is included");
        let continent = terrain.find("if (t.continent").expect("continents are included last");
        assert!(feature < ground && ground < continent, "feature, terrain, continent order");
        let development_start = terrain_end;
        let development_end = EMBEDDED_INDEX[development_start..]
            .find("\nfunction tileTipLines")
            .map(|offset| development_start + offset)
            .expect("the resource formatter ends before details");
        let development = &EMBEDDED_INDEX[development_start..development_end];
        assert!(
            development.contains("function tileResourceTipLine(t)")
                && development.contains("Resource: "),
            "resources are named after the terrain"
        );
        for emoji in ["●", "🥾", "🛡", "🌸", "👁", "♜", "✦", "⌂", "⬡", "🏗", "🛤", "🏛", "⚑", "⚡"] {
            assert!(
                !details.contains(emoji),
                "map details should use text and map yield markers, not {emoji}"
            );
        }
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

    /// The primary map controls read left to right in the order a viewer
    /// reaches for them: collapse the command deck, set the map area, face
    /// north, zoom in, zoom out, and dismiss. When relevant, the sky route is
    /// a second row beneath that primary row.
    /// Dismissal is last in that primary row because it is the only one that
    /// removes the bar, and it hides itself while the deck is collapsed —
    /// Display Settings is the only way back and it lives inside the deck.
    #[test]
    fn the_map_controls_run_from_collapse_to_dismiss() {
        let dock = EMBEDDED_INDEX
            .split_once("<div id=\"map-controls-dock\">")
            .expect("the map control dock")
            .1
            .split_once("<div id=\"map-area-editor\"")
            .expect("the end of the map control dock")
            .0;
        let mut previous = 0usize;
        let mut last = "the start of the dock";
        for control in [
            "id=\"paneltoggle\"",
            "id=\"mapareaset\"",
            "id=\"compass\"",
            "id=\"zin\"",
            "id=\"zout\"",
            "data-overlay-close=\"controls\"",
            "id=\"skynav\"",
        ] {
            let at = dock
                .find(control)
                .unwrap_or_else(|| panic!("the map controls are missing {control}"));
            assert!(
                at > previous,
                "the map controls run in reading order: {control} must follow {last}"
            );
            previous = at;
            last = control;
        }
        assert!(
            EMBEDDED_INDEX.contains(
                r#"body.sidebar-hidden .overlay-close[data-overlay-close="controls"] { display: none; }"#
            ),
            "the dismiss control must go while the deck that restores it is collapsed"
        );
    }

    /// Gearing the wheel made fourteen orders of magnitude *reachable*; it did
    /// not make them navigable. Sixty-five notches is a distance nobody
    /// scrolls, and every one of them has to be aimed over empty space. So the
    /// sky carries its own way about: the places the gearing already calls
    /// arrivals, the shots that hold a whole scale, and the zoom laid out end
    /// to end as a ladder — one complete route inside the map controls, up
    /// only while there is a sky to cross.
    #[test]
    fn the_sky_carries_its_own_way_about() {
        // It is part of the map controls: the same dock and dismissal, in a
        // second row below the primary controls. The complete labelled route
        // stays together rather than splitting compact world glyphs by the
        // compass from the solar-system route above the map.
        let dock = EMBEDDED_INDEX
            .split_once("<div id=\"map-controls-dock\">")
            .expect("the map control dock")
            .1;
        let bar = dock.find("id=\"skynav\"").expect("the sky navigator");
        let zoom = dock.find("<div id=\"zoomctl\">").expect("the zoom controls");
        let minus = dock.find("id=\"zout\"").expect("the zoom-out control");
        let exit = dock
            .find("data-overlay-close=\"controls\"")
            .expect("the map-control dismissal");
        assert!(
            zoom < bar,
            "the sky navigator belongs inside the map-control strip"
        );
        assert!(
            minus < exit && exit < bar,
            "the primary row dismisses before the second-row sky route"
        );
        assert!(EMBEDDED_INDEX.contains("#skynav[hidden] { display: none; }"));
        assert!(EMBEDDED_INDEX.contains("#zoomctl-main"));
        assert!(EMBEDDED_INDEX.contains("flex-direction: column;"));
        assert!(EMBEDDED_INDEX.contains("position: static; z-index: auto; align-self: flex-start;"));
        assert!(EMBEDDED_INDEX.contains("syncSkyNavDockGeometry();"));
        for part in [
            "id=\"skynav-worlds\"",
            "id=\"skynav-scales\"",
            "id=\"skyladder\"",
            "id=\"skyspan\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(part),
                "the sky navigator is missing {part}"
            );
        }
        assert!(
            !EMBEDDED_INDEX.contains("id=\"skyworlds\""),
            "the compact icon-only skyworlds strip is gone"
        );
        // Only home takes the bar down, and only while home fills the frame:
        // home is the one world with a board on it. Every other world is
        // somewhere to get back off, and two readings of that rule cost a pass
        // each. Keyed to home's *nominal* size the bar vanished the moment a
        // jump arrived anywhere but home — on Mars the Earth is drawn two
        // stages wide and is also a hundred million kilometres away and not on
        // the screen at all — and keyed to whatever the camera focused on it
        // vanished at the destination, because the star there is twelve times
        // its own planet, sits at the same catalogue point, and is not even
        // painted at that zoom.
        assert!(EMBEDDED_INDEX.contains("if (!focus || focus.body.id !== \"earth\") return true;"));
        assert!(EMBEDDED_INDEX
            .contains("return 2 * focus.drawn < stage * (SKY_NAV_UP ? 1.05 : .9);"));
        assert!(!EMBEDDED_INDEX.contains("const across = 2 * skyDrawnRadius(SKY_EARTH);"));
        // And it is synced before the branch, so it comes down when the world
        // under it is a flat board as surely as it goes up above a globe.
        let sync = EMBEDDED_INDEX
            .find("  syncSkyNav();")
            .expect("the sky navigator is synced from the renderer");
        let branch = EMBEDDED_INDEX
            .find("if (drawPlanetMap()) return;")
            .expect("the planet branch");
        assert!(sync < branch, "the sky navigator syncs before the renderer branches");

        // The places are the arrivals and nothing else — one list, so the bar
        // can never name somewhere the gearing does not treat as a landing —
        // and only what this civilization may look at.
        assert!(EMBEDDED_INDEX.contains("  for (const id of SKY_ARRIVALS) {"));
        assert!(EMBEDDED_INDEX
            .contains("const known = new Set(skyKnownWorlds().map(body => body.id));"));
        assert!(EMBEDDED_INDEX
            .contains("const SKY_STOP_MARKS = {earth:\"◉\", moon:\"☾\", mars:\"♂\", exo:\"✧\"};"));
        // A named shot is the same arithmetic as the stop the zoom already
        // stops at, taken out of `skySystemFrame` rather than written twice.
        assert!(EMBEDDED_INDEX.contains("return skyFrameFor(skyOutermostFrame());"));
        assert!(EMBEDDED_INDEX.contains("function skyFrameFor(box)"));
        assert!(EMBEDDED_INDEX.contains("function skyStopScale(stop)"));
        assert!(EMBEDDED_INDEX.contains("  if (knowsNeighbourhood())"));
        // The sky stops at twenty light-years, and the stop is written as the
        // **width of the stage** rather than as a reach — which is the number
        // the bar prints under it. Every other shot out here is a radius inside
        // the padded frame, and a stop written that way printed `45.7 ly`
        // beneath itself: a view arguing with its own caption. `skyFrameFor`
        // and `skyReachForStageWidth` are the two directions of one conversion,
        // so the stop and the readout cannot drift apart.
        assert!(EMBEDDED_INDEX.contains("const SKY_STOP_LY = 20;"));
        assert!(EMBEDDED_INDEX.contains("function skyStopReach()"));
        assert!(EMBEDDED_INDEX.contains("function skyReachForStageWidth(width)"));
        assert!(EMBEDDED_INDEX.contains("function skyFramedBox()"));
        assert!(EMBEDDED_INDEX.contains(
            "  return width * Math.min(inside.width, inside.height) / (2 * skyStageSpan());"
        ));
        // And the drawn shell is half of it, so the ring spans the stage rather
        // than hanging off the edge of it.
        assert!(EMBEDDED_INDEX.contains("const SKY_BUBBLE_LY = SKY_STOP_LY / 2;"));
        // The top rung's stop is the voyage rather than a scale: the Sun at one
        // end and the route out towards the destination. It decides where the
        // camera leans, never how far it pulls back — a stop that quietly opens
        // whenever the expedition is aimed a long way off is not a stop — and
        // the lean is capped against the *stage*, which is not the same length
        // as the reach.
        assert!(EMBEDDED_INDEX.contains("  if (seesDestination()) return skyVoyageFrame();"));
        assert!(EMBEDDED_INDEX.contains("function skyVoyageFrame()"));
        assert!(EMBEDDED_INDEX.contains("  const room = SKY_STOP_LY * LIGHT_YEAR * .4;"));
        // Centred a *third* of the way along the trip rather than half. This is
        // the view the race is watched from: the expedition leaves the Sun and
        // crawls outward over the rest of the game, so what is being tracked
        // spends almost all of its life at the near end of the route, and the
        // far end is a world that is not going anywhere. On the midpoint half
        // the stage went to the empty side of a dot.
        assert!(EMBEDDED_INDEX.contains("const SKY_VOYAGE_ALONG = 1 / 3;"));
        assert!(EMBEDDED_INDEX.contains(
            "  const dx = (ex - sx) * SKY_VOYAGE_ALONG, dy = (ey - sy) * SKY_VOYAGE_ALONG;"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "  const lean = Math.min(1, room / Math.max(1e-9, Math.hypot(dx, dy)));"
        ));
        assert!(EMBEDDED_INDEX.contains("  return {x:sx + dx * lean, y:sy + dy * lean, reach};"));
        // Two shots, named as the places they are: the solar system, and the
        // voyage. The third was the galaxy and it is gone with the picture it
        // framed — every constant, every trace and the caption that read off
        // them, so nothing is left drawing a hundred thousand light-years that
        // no zoom can now reach.
        assert!(EMBEDDED_INDEX.contains("label:\"Solar System\", mark:\"☉\""));
        assert!(EMBEDDED_INDEX.contains("stops.push({id:\"voyage\", label:\"Voyage\""));
        // The complete old route lives in the control row in the order a
        // journey reads: the local arrivals Earth, Moon, and Mars; the solar
        // system and Voyage shots; then the destination called Exoplanet. The
        // destination is still a world flight, even though it follows the
        // scale shots in the row.
        assert!(EMBEDDED_INDEX.contains(
            "const local = worlds.filter(stop => stop.id !== \"exo\");"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const onward = [...scales, ...worlds.filter(stop => stop.id === \"exo\")];"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "skyNavWorldRow.replaceChildren(...local.map(stop => skyNavButton(\"world\", stop)));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "skyNavScaleRow.replaceChildren(...onward.map(stop =>"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "skyNavButton(stop.body ? \"world\" : \"scale\", stop)"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "document.querySelectorAll(\"#skynav [data-sky-stop]\")"
        ));
        assert!(EMBEDDED_INDEX.contains("button.setAttribute(\"aria-label\", stop.label);"));
        // A switch takes three times as long as it first shipped at, both ways.
        // The distances are the content, and at the old pace the crossing was
        // over before it had said anything about what lay between the two ends.
        assert!(EMBEDDED_INDEX.contains("const SKY_TRAVEL_PACE = 3;"));
        assert!(EMBEDDED_INDEX.contains("  return Math.max(420 * SKY_TRAVEL_PACE,"));
        assert!(!EMBEDDED_INDEX.contains("return Math.max(420, Math.min(1700, S * 240));"));
        for gone in [
            "GALAXY_DISC_LY",
            "GALAXY_SUN_RADIUS_LY",
            "GALAXY_ARMS",
            "GALAXY_SPUR",
            "function drawSkyGalaxy",
            "function galaxySpeckle",
            "function drawSkyHere",
            "The Milky Way",
            "The Orion Spur",
            "label:\"Galaxy\"",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(gone),
                "the galaxy view is dropped, but {gone} is still in the client"
            );
        }
        // These controls stay practical and stable: each button is named only
        // for its destination, with no changing survey name or hover copy.
        assert!(EMBEDDED_INDEX
            .contains("const SKY_STOP_LABELS = {earth:\"Earth\", moon:\"Moon\", mars:\"Mars\", exo:\"Exoplanet\"};"));
        assert!(EMBEDDED_INDEX.contains(
            "                label:SKY_STOP_LABELS[id] || body.name.replace(/^The /, \"\")});"
        ));
        assert!(EMBEDDED_INDEX.contains("button.title = stop.label;"));
        assert!(!EMBEDDED_INDEX.contains("Fly to ${body.name}"));
        // The labels are fixed, so they are all the rebuild key needs.
        assert!(EMBEDDED_INDEX
            .contains(".map(stop => `${stop.id}/${stop.label}`).join(\",\");"));
        assert!(!EMBEDDED_INDEX.contains("function skySceneCaption"));
        assert!(EMBEDDED_INDEX.contains("function mapControlsBox(viewRect)"));
        // Short of the ceiling on purpose: a jump that lands exactly on the
        // stop leaves the zoom-in button dead in the hand on arrival.
        assert!(EMBEDDED_INDEX.contains("const SKY_STOP_FILL = .8;"));

        // A jump is a path with a shape, not an ease toward a target. Easing a
        // zoom and a pan together across fourteen orders of magnitude spends
        // the whole middle of the trip over empty space with nothing on the
        // stage; Van Wijk and Nuij's path pulls back until both ends are in
        // view, crosses, and comes back down.
        assert!(EMBEDDED_INDEX.contains("function skyTravelPath(from, w0, to, w1)"));
        assert!(EMBEDDED_INDEX.contains("const SKY_TRAVEL_RHO = 1.42;"));
        assert!(EMBEDDED_INDEX.contains("cameraZoom = {kind:\"planet\", scale:want, pan:to, lean:null,"));
        // `ln(-b + sqrt(b*b + 1))` has to be written as `-asinh(b)`: out here
        // `b` reaches 1e15, `sqrt(b*b + 1)` is exactly `b` in float64, the
        // subtraction cancels to zero and the whole flight comes out NaN.
        assert!(EMBEDDED_INDEX.contains("const r0 = -Math.asinh(b0), r1 = -Math.asinh(b1);"));
        assert!(!EMBEDDED_INDEX.contains("Math.log(-b0 + Math.sqrt"));

        // The ladder is the gearing's own ruler with a grip on it, so it is
        // not geared again — and it does not go through `zoomAt`, which stops
        // a zoom in at the ceiling of the world nearest the *camera*. That is
        // right for one notch and catastrophic for a whole ladder in one move:
        // at the far stop the nearest world is some red dwarf, its ceiling is
        // a hundredth, and dragging the handle to the ground put the camera
        // there and left it. The ladder's own top is the world it leads to.
        assert!(EMBEDDED_INDEX.contains("function skyLadderSubject()"));
        assert!(EMBEDDED_INDEX.contains("function skyLadderPan(scale, subject)"));
        assert!(EMBEDDED_INDEX.contains(
            "                        Math.min(planetMaxScale(SKY_PAN, subject), skyLadderScale(along)));"
        ));
        assert!(!EMBEDDED_INDEX.contains("zoomAt(want / base);"));
        assert!(EMBEDDED_INDEX.contains("Math.log(Math.max(floor, scale) / floor) / ladder"));
        // The road leads somewhere even when the camera is not standing on a
        // stop, and that somewhere is the nearest one — never unconditionally
        // home. `skyNavHere` wants an arrival above .55, which one notch of the
        // ladder falls below, so a subject that fell back to the Earth handed
        // the road home the instant anybody touched the zoom after arriving at
        // the destination: measured on `origin/main`, the exoplanet was in
        // frame at 1 of 11 rungs and ended 115 billion pixels off centre with
        // the caption reading "The Earth and the Moon". It is the same answer
        // as before wherever home really is the nearest place.
        assert!(!EMBEDDED_INDEX.contains("  return stop ? stop.body : SKY_EARTH;"));
        assert!(EMBEDDED_INDEX.contains("    if (!best || away < best.away) best = {away, body:candidate.body};"));
        // And the camera may not stand more than a third of a stage from that
        // subject. Keying the pan to the subject's *drawn size* alone left it
        // centred on the far stop for every rung below 2% of the stage, which
        // out here is most of the road: the ladder zoomed into empty sky
        // between here and the destination and never showed the destination.
        assert!(EMBEDDED_INDEX.contains("  if (away > 0) ease = Math.max(ease, 1 - span * .35 / away);"));
        // And a star is not something anybody turns. `skyFocusBody` takes the
        // largest disc over the stage, and at the destination the star is
        // eleven times its own planet at the *same catalogue point* — so every
        // drag made on the exoplanet grabbed the star instead and turned a
        // ball with no ground on it. Measured on `origin/main`: the grab is
        // `luyten`, 5,959 pixels wide, and the planet's longitude never moves.
        // A star has no `frequency`, so `skyGroundFade` never gives it one.
        assert!(EMBEDDED_INDEX.contains("    if (body.kind === \"star\") continue;"));

        // The zoom buttons repeat while held. On the flat board that is a
        // convenience; out here it is the difference between a control and an
        // ornament.
        assert!(EMBEDDED_INDEX.contains("function bindHeldZoom(id, step)"));
        assert!(EMBEDDED_INDEX.contains(
            "bindHeldZoom(\"zin\", () => { takeCameraControl(true); zoomAt(skyZoomStep(1.35)); });"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "bindHeldZoom(\"zout\", () => { takeCameraControl(true); zoomAt(skyZoomStep(1 / 1.35)); });"
        ));

        // The eased planet zoom's divisor has to sit below every scale that can
        // really be standing there, and `1e-6` did not: the sky past about a
        // hundred AU is a smaller number than that, so out there the ratio
        // stopped tracking the camera and became a constant. Measured on
        // unmodified `main`, seventy wheel notches out from a tile came to rest
        // at 2.2e-155 against a far stop of 3.7e-15 — a hundred and forty
        // orders of magnitude past the end of the sky, still zooming, and it
        // never stops. It is the pinch's `1e-4` again, one guard along.
        assert!(EMBEDDED_INDEX.contains(
            "cam.scale *= Math.exp(Math.log(cameraZoom.scale / Math.max(1e-30, cam.scale)) * ease);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const scaleLeft = Math.abs(Math.log(cameraZoom.scale / Math.max(1e-30, cam.scale)));"
        ));
        assert!(!EMBEDDED_INDEX.contains("Math.max(1e-6, cam.scale)"));

        // The practical ruler sits beside the map controls whenever that space
        // fits, and otherwise rises above them without covering a button.
        assert!(EMBEDDED_INDEX.contains("  const nav = skyNavBox(viewRect);"));
        assert!(EMBEDDED_INDEX.contains("const controls = mapControlsBox(viewRect);"));
        assert!(EMBEDDED_INDEX.contains(
            "const right = mapScaleRightEdge(viewRect, controls, frame.right - 12);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const beside = controls && controls.right + 14 + width <= right;"
        ));
    }

    /// Every non-Sun body in the catalogue that has a frequency is a tiled
    /// visual surface. Their resource marks use already-known Civ resource IDs,
    /// but remain separate from the game rules until an off-world economy is
    /// deliberately designed.
    #[test]
    fn every_tiled_sky_surface_is_zoomable_without_navigation_buttons() {
        let worlds = [
            "mercury", "venus", "moon", "mars", "ceres", "jupiter", "io", "europa",
            "ganymede", "callisto", "saturn", "titan", "uranus", "neptune", "pluto",
        ];
        for world in worlds {
            let spec = EMBEDDED_INDEX
                .split_once(&format!("id:\"{world}\""))
                .unwrap_or_else(|| panic!("{world} is absent from the sky catalogue"))
                .1
                .split_once("},")
                .unwrap_or_else(|| panic!("{world} sky catalogue entry is not closed"))
                .0;
            assert!(spec.contains("frequency:"), "{world} must have a tiled surface");
        }
        let surface_list = EMBEDDED_INDEX
            .split_once("const SKY_SURFACE_WORLDS = [")
            .expect("the tiled surface world list")
            .1
            .split_once("];\nconst SKY_ZOOMABLE_WORLDS")
            .expect("the tiled surface list terminator")
            .0;
        for world in worlds {
            assert!(surface_list.contains(&format!("\"{world}\"")), "{world} is not zoomable");
        }
        assert!(EMBEDDED_INDEX.contains(
            "const SKY_ZOOMABLE_WORLDS = [...SKY_ARRIVALS, ...SKY_SURFACE_WORLDS];"
        ));
        assert!(EMBEDDED_INDEX.contains("const ids = [...SKY_ZOOMABLE_WORLDS];"));
        assert!(EMBEDDED_INDEX.contains("function skyResourceCells(body, cells)"));
        assert!(
            EMBEDDED_INDEX.contains("function drawSkyResourceBadge(resource, x, y, size, alpha)")
        );
        assert!(EMBEDDED_INDEX.contains("drawResourcePictogram(resource, x, y, size * 1.35);"));

        // A world stop is a navigation button. These bodies are deliberately
        // excluded from that list even though their zoom approach is paced.
        let world_stops = EMBEDDED_INDEX
            .split_once("function skyWorldStops()")
            .expect("the sky navigation stop builder")
            .1
            .split_once("function skyScaleStops()")
            .expect("the sky scale stop builder")
            .0;
        assert!(world_stops.contains("for (const id of SKY_ARRIVALS)"));
        assert!(!world_stops.contains("SKY_ZOOMABLE_WORLDS"));
    }

    #[test]
    fn solar_rings_are_planet_only_and_the_sun_uses_a_cached_photosphere() {
        let orbit_list = EMBEDDED_INDEX
            .split_once("const SKY_OFFICIAL_PLANETS = new Set([")
            .expect("the official planet orbit list")
            .1
            .split_once("]);\n")
            .expect("the official planet orbit list terminator")
            .0;
        for planet in [
            "mercury", "venus", "earth", "mars", "jupiter", "saturn", "uranus", "neptune",
        ] {
            assert!(orbit_list.contains(&format!("\"{planet}\"")), "{planet} needs a solar ring");
        }
        for dwarf in ["ceres", "pluto"] {
            assert!(!orbit_list.contains(&format!("\"{dwarf}\"")), "{dwarf} is a dwarf planet");
        }
        assert!(EMBEDDED_INDEX.contains("body.parent || SKY_OFFICIAL_PLANETS.has(body.id)"));
        assert!(EMBEDDED_INDEX.contains("let SKY_SUN_TEXTURE = null;"));
        assert!(EMBEDDED_INDEX.contains("function skySunTexture()"));
        assert!(EMBEDDED_INDEX.contains("function drawSkySunBall(place, alpha)"));
        assert!(EMBEDDED_INDEX.contains("if (place.light) return drawSkySunBall(place, alpha);"));
    }

    /// The world is drawn into a rectangle of the map area rather than into
    /// all of it, so a viewer moves the map out from under the panels instead
    /// of dragging the world around underneath them. The canvas, the
    /// vignettes and the editor's own edges take their box from one set of
    /// custom properties, every renderer measures in `MAPW`/`MAPH` rather
    /// than in the container, and the automatic fit is the same uncovered
    /// rectangle the camera already composes into — one measurement, not two
    /// that can disagree.
    #[test]
    fn the_map_area_is_a_rectangle_the_viewer_can_set() {
        for piece in [
            "--map-area-left: 0px;",
            "--map-area-width: 100%;",
            "function uncoveredMapBox(origin, width, height)",
            "function syncMapViewport()",
            "function refitMapAreaToChrome()",
            "function moveMapAreaEdgeTo(edge, clientX, clientY)",
            "civvis-map-area-v1",
            "id=\"map-area-editor\"",
            "id=\"map-area-apply\"",
            "id=\"map-area-cancel\"",
            "id=\"map-area-reset\"",
            "body.map-area-inset #map",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the map area is missing {piece}"
            );
        }
        for edge in ["top", "bottom", "left", "right"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("data-map-edge=\"{edge}\"")),
                "the map area must be set by dragging its {edge} edge"
            );
            assert!(
                EMBEDDED_INDEX.contains(&format!("data-shade=\"{edge}\"")),
                "the ground outside the map area's {edge} edge must be dimmed"
            );
        }
        // The fitted world runs to the bottom of the screen. The zoom dock
        // floats over it like every other lower instrument, so the uncovered
        // rectangle must not carve a strip out for it.
        let uncovered = EMBEDDED_INDEX
            .split_once("function uncoveredMapBox(origin, width, height)")
            .expect("the uncovered-map measurement")
            .1
            .split_once("\nfunction ")
            .expect("the end of the uncovered-map measurement")
            .0;
        assert!(
            !uncovered.contains("map-controls-dock"),
            "the map fit must ignore the floating map-controls dock"
        );
        // The canvas is placed by those properties rather than stretched to
        // its container. `width: 100%` on #map is exactly what this replaces.
        let map_rule = EMBEDDED_INDEX
            .split_once("  #map {")
            .expect("the map canvas rule")
            .1
            .split_once('}')
            .expect("the end of the map canvas rule")
            .0;
        for property in [
            "left: var(--map-area-left)",
            "top: var(--map-area-top)",
            "width: var(--map-area-width)",
            "height: var(--map-area-height)",
        ] {
            assert!(
                map_rule.contains(property),
                "the map canvas must take its {property} from the map area"
            );
        }
        assert!(
            !map_rule.contains("width: 100%"),
            "a canvas stretched to its container cannot be moved off the panels"
        );
        // Renderers and camera measure the viewport, never the container.
        assert!(EMBEDDED_INDEX.contains("const vx = (sx - MAPW / 2) / cam.scale;"));
        assert!(EMBEDDED_INDEX.contains("cv.width !== backingWidth"));
        // The fit is on out of the box, in the stored default and in the
        // switch that reports it, and moving an edge by hand turns it off.
        assert!(EMBEDDED_INDEX.contains("const MAP_AREA_DEFAULT = {auto:true,"));
        assert!(EMBEDDED_INDEX
            .contains(r#"<input type="checkbox" id="map-area-auto" checked>"#));
        assert!(EMBEDDED_INDEX.contains("if (!MAP_AREA.auto || mapAreaRefitDepth) return false;"));
        // Every place a panel or an overlay moves refits a fitted area.
        assert_eq!(
            EMBEDDED_INDEX.matches("refitMapAreaToChrome();").count(),
            5,
            "a fitted map area follows the standings, the overlay switches, and \
             both HUD layout paths — five call sites; a sixth means a new one \
             belongs in this count, a fourth means one was dropped"
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
    let current_seed = session.game.seed;
    let shared = Arc::new(Shared {
        current_seed: AtomicU64::new(current_seed),
        supervisor_request: Mutex::new(None),
        live_params: Mutex::new(session.params.clone()),
        next_game_params: Mutex::new(session.take_resumed_next_game_params()),
        session: Mutex::new(session),
        pace_ms: AtomicU64::new(500), // half a second per turn by default
        between_game_countdown_ms: AtomicU64::new(DEFAULT_BETWEEN_GAME_COUNTDOWN_MS),
        paused: AtomicBool::new(initially_paused),
        restart_in: AtomicU64::new(u64::MAX),
        turn_us: AtomicU64::new(0),
        turn_compute_us: AtomicU64::new(0),
        frame_sequence: AtomicU64::new(0),
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
    for mut s in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || handle(&mut s, &shared));
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
