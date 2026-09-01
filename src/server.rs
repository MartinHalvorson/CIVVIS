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
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::ai::{AdvancedAi, Ai, BasicAi};
use crate::civ6;
use crate::game::{Action, Game, GameOptions, LeaderPool, PlayOnMode, VictoryConditions};
use crate::leader_roster;
use crate::name::Name;
use crate::obs::{observation, observation_player_view, observation_spectator};
use crate::rules::Rules;
use crate::setup::{
    battlefield_sizes, future_era_from_id, future_era_id, start_era_from_id, start_era_id,
    turn_structure_id, BaseRuleset, FutureEra, GameSpeed, MapPoles, MapScript, MapSize,
    MapTopology, TacticsEra, TacticsRules, TurnStructure,
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

/// A panic anywhere in the request-handling thread while it holds one of
/// these locks — malformed input reaching an `unwrap` deeper in the call
/// tree, say — poisons the mutex. `Mutex::lock` then errors forever after,
/// which used to take the whole exhibition offline: one bad request killed
/// every request behind it. The data behind a poisoned lock was left in
/// whatever state it was in the instant its holder panicked, which for every
/// mutex in this file is a plain value type with no invariant that spans two
/// fields, so reading it back is safe; recover the guard and keep serving.
fn lock_or_recover<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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
    let mut held = lock_or_recover(cache);
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
    let mut held = lock_or_recover(PREVIOUS.get_or_init(|| Mutex::new(None)));
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
    let output = Command::new("top")
        .args(["-l", "1", "-n", "0"])
        .output()
        .ok()?;
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
    (total > 0 && available <= total).then(|| 100.0 * (total - available) as f64 / total as f64)
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
            let output = Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
                .ok()?;
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
const EMBEDDED_APP_PALETTE_JS: &str = include_str!("../web/assets/app_palette.js");
const EMBEDDED_APP_JS: &str = include_str!("../web/assets/app.js");
/// The whole page source — the document plus its one external script — as a
/// single searchable string. Only the source-contract tests read it: they
/// assert against this combined source, which is the same contract they
/// checked when the script block was inline. The server itself ships the two
/// parts separately at `/` and `/assets/app.js`. (The split exists because
/// the script was a 1.35 MB block inside `web/index.html`, making that file
/// the repository's largest history payer at ~1 MB of pack growth per edit.)
#[cfg(test)]
static EMBEDDED_INDEX: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    // ⚠ EVERY SCRIPT THE PAGE LOADS. A test that reads "the client" and gets
    // only some of its scripts asserts about a program the browser never runs.
    // Carving the palette out of app.js broke six of these at once for exactly
    // that reason — the assertions were about terrain colours and tile art that
    // had simply moved to the script beside it.
    let mut page = String::with_capacity(
        EMBEDDED_INDEX_HTML.len() + EMBEDDED_APP_PALETTE_JS.len() + EMBEDDED_APP_JS.len(),
    );
    page.push_str(EMBEDDED_INDEX_HTML);
    page.push_str(EMBEDDED_APP_PALETTE_JS);
    page.push_str(EMBEDDED_APP_JS);
    page.push_str(EMBEDDED_APP_SETUP_JS);
    page
});
const EMBEDDED_APP_SETUP_JS: &str = include_str!("../web/assets/app_setup.js");
const EMBEDDED_FEATURE_ATLAS: &[u8] = include_bytes!("../web/assets/feature-atlas.png");
const EMBEDDED_ENVIRONMENT_FEATURE_ATLAS: &[u8] =
    include_bytes!("../web/assets/environment-feature-atlas.png");
const EMBEDDED_HIDDEN_MAP_MONSTERS: &[u8] = include_bytes!("../web/assets/hidden-map-monsters.png");
const EMBEDDED_CIV6_UNIT_FLAGS: &[u8] = include_bytes!("../web/assets/civ6-unit-flags.png");
const EMBEDDED_CIV6_YIELD_ICONS: &[u8] = include_bytes!("../web/assets/civ6-yield-icons.png");
const EMBEDDED_CIV6_UNIT_FLAG_PLATES: &[u8] =
    include_bytes!("../web/assets/civ6-unit-flag-plates.png");
const EMBEDDED_CIV6_CITY_BANNER_SHIELDS: &[u8] =
    include_bytes!("../web/assets/civ6-city-banner-shields.png");

/// The agents that exist in every build, with a friendly handle each.
/// `crate::elo::builtin_send_ai` resolves the id, and the auto-play control
/// offers this list.
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
    /// The Mercy Rule threshold for new games, if any. See `Game::mercy_rule`.
    pub mercy_rule: Option<f64>,
    /// Distinct victory types required to win. See
    /// `Game::effective_required_victories`.
    pub required_victory_types: usize,
    /// What a Tactics arena grants its two sides. Read only when the map
    /// script is the battlefield; a world earns its yields instead.
    pub tactics: TacticsRules,
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
    /// The built-in agent a player handed their own seat to, by name.
    autoplay_strategy: Option<String>,
    /// The last browser batch that borrowed the human seat, and how many
    /// turns it played. A client retries the same id after a dropped socket;
    /// remembering one completed batch makes that retry an acknowledgement,
    /// not a second run.
    last_autoplay_request: Option<(String, usize)>,
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
    /// How many planning workers the next simultaneous cycle may use.
    ///
    /// One everywhere by default: the browser build has no threads, and a
    /// game a person is playing must not commandeer their machine. The
    /// exhibition pacer raises it while the pace is Lightning and lowers it
    /// back when a viewer slows down. The game is byte-for-byte identical at
    /// any count — the dial buys throughput, never different play.
    simultaneous_jobs: usize,
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
    /// The Tactics match in progress, when the arena is being played as a
    /// series rather than as one battle.
    ///
    /// It lives here rather than on the `Session` because a match outlives
    /// the battles it is made of, and `start_automatic_next_game` replaces
    /// the whole session between them.
    match_series: Mutex<Option<crate::setup::MatchSeries>>,
    pub pace_ms: AtomicU64,
    /// How long a completed spectator game remains on its result screen.
    /// This is a viewer preference, not part of the simulated world: each
    /// browser persists and reasserts its choice when the supervisor starts a
    /// fresh process.
    pub between_game_countdown_ms: AtomicU64,
    /// Set when the between-game countdown is changed while a finished game
    /// is being held on its result screen: the stepper then counts the new
    /// length from that moment rather than from the game's end. A shorter
    /// choice is a request for the next world sooner, not for it now, and a
    /// longer one is a request for more time, not for what is left of it.
    pub finale_rearm: AtomicBool,
    /// Which hold the countdown belongs to: bumped every time a hold starts
    /// or starts over, and published beside `restart_in` so a viewer's own
    /// clock can tell a re-armed hold — a deadline that has genuinely moved
    /// later — from a late or duplicate snapshot of the same hold, which it
    /// must never let move a countdown backwards.
    pub finale_hold: AtomicU64,
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
/// The unlimited pace still hands the accept loop a slot this often, so the
/// page keeps loading state while the stepper runs flat out.
const UNLIMITED_BREATH_MS: u64 = 100;
/// Blitz is the first watch pace where every player's completed turn becomes
/// its own frame. Faster custom paces retain Lightning's one-frame-per-round
/// throughput; every offered pace at or above this threshold is seat-by-seat.
const PLAYER_TURN_FRAME_MIN_PACE_MS: u64 = 500;
/// Minor civilizations and barbarians take a quarter of a major's slice.
const MINOR_SHARE: f64 = 0.25;

fn publishes_player_turn_frames(pace_ms: u64) -> bool {
    pace_ms >= PLAYER_TURN_FRAME_MIN_PACE_MS
}

/// The planning fleet Lightning turns loose on a simultaneous world: nine
/// cores in every ten, so a tenth of the machine stays free for whatever
/// else the host is running. Any actual pace keeps the serial cycle — a
/// paced turn spends its wall clock on the budget, not on compute — and a
/// sequential world ignores the figure entirely, because its seats each
/// act on the world the previous seat left and cannot deliberate together.
fn simultaneous_jobs_for(pace_ms: u64) -> usize {
    if pace_ms != 0 {
        return 1;
    }
    static FLEET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *FLEET.get_or_init(|| {
        std::thread::available_parallelism().map_or(1, |cores| nine_tenths_of(cores.get()))
    })
}

/// Nine tenths rounded down, never below one worker.
fn nine_tenths_of(cores: usize) -> usize {
    (cores * 9 / 10).max(1)
}

fn spectator_frame(game: &Game, sequence: u64) -> SpectatorFrame {
    SpectatorFrame {
        seed: game.seed,
        turn: game.turn,
        finished: game.is_finished(),
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
        || game.is_finished() != finished_before
}

/// The result countdown can only use the four values the viewer can select.
pub(crate) fn valid_between_game_countdown_ms(value: u64) -> Option<u64> {
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
        *lock_or_recover(&self.latest) = Some(frame);
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
        let mut latest = lock_or_recover(&self.latest);
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
                let session = lock_or_recover(&self.session);
                spectator_frame(&session.game, self.frame_sequence.load(Ordering::Relaxed))
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
        lock_or_recover(&self.frame_delivery).frame_delivered(viewer, frame, Instant::now());
    }

    fn note_viewer_request(&self, viewer: &str, painted: Option<SpectatorFrame>) {
        lock_or_recover(&self.frame_delivery).viewer_request(viewer, painted, Instant::now());
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
        let mut delivery = lock_or_recover(&self.frame_delivery);
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
        let mut delivery = lock_or_recover(&self.frame_delivery);
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
        let base = lock_or_recover(&self.live_params).clone();
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
            let session = lock_or_recover(&self.session);
            let human_was_eliminated = !session.params.spectate
                && session
                    .game
                    .players
                    .get(0)
                    .is_some_and(|player| !player.alive);
            if session.game.seed != expected_seed
                || (!session.game.is_finished() && !human_was_eliminated)
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
        *lock_or_recover(&self.supervisor_request) = Some(json!({
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
        let base = lock_or_recover(&self.live_params).clone();
        let (params, _) = crate::routes::next_game_settings(&base, request);
        *lock_or_recover(&self.next_game_params) = Some(params);
    }

    /// What the next world will be started from, as the setup panel reads it.
    fn staged_next_game_settings(&self) -> Value {
        lock_or_recover(&self.next_game_params)
            .as_ref()
            .map(simulation_settings)
            .unwrap_or(Value::Null)
    }

    /// Hand the queue to the world that is about to consume it.
    fn take_next_game_params(&self) -> Option<Params> {
        lock_or_recover(&self.next_game_params).take()
    }

    /// The Tactics match in progress, for `/state`.
    fn match_series(&self) -> Option<crate::setup::MatchSeries> {
        lock_or_recover(&self.match_series).clone()
    }

    /// Carry a Tactics match on past the battle that just finished.
    ///
    /// Records the result, then hands the next battle the same two
    /// civilizations with the sides swapped over — which is the whole point
    /// of a series, because a match in which one civilization held the same
    /// corner every time would be measuring the corner. A match that has
    /// been settled is cleared here and the next battle opens a fresh one, so
    /// an exhibition left running plays match after match rather than one
    /// endless tally.
    fn advance_match(&self, finished: &Game, next: Option<Params>) -> Option<Params> {
        let mut next = next?;
        let mut held = lock_or_recover(&self.match_series);
        if !next.map_script.is_battlefield() || next.tactics.best_of <= 1 {
            *held = None;
            return Some(next);
        }
        let contenders: Vec<String> = finished
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.civ.clone())
            .collect();
        let fresh = || crate::setup::MatchSeries::new(next.tactics.best_of, contenders.clone());
        let series = held.get_or_insert_with(fresh);
        if series.decided() {
            // The battle before this one settled the match, and its final
            // scoreline has had its turn on the result screen.
            *series = fresh();
        } else {
            series.record(
                finished
                    .winner
                    .and_then(|pid| finished.players.get(pid))
                    .map(|player| player.civ.as_str()),
            );
        }
        // The pair is the match's, in a fixed order, so "which end" is
        // decided by how many battles have been played and nothing else.
        let mut pair: Vec<String> = series.wins.keys().cloned().collect();
        if series.played() % 2 == 1 {
            pair.reverse();
        }
        next.civs = pair;
        Some(next)
    }

    /// The request the supervisor is being asked to act on, if any.
    fn pending_new_game_request(&self) -> Option<Value> {
        lock_or_recover(&self.supervisor_request).clone()
    }

    /// Adopt the settings a freshly started world runs under, so the next
    /// new-game request can be normalized against them without the lock.
    fn adopt_live_params(&self, params: &Params) {
        *lock_or_recover(&self.live_params) = params.clone();
    }

    /// Whether any viewer still present is owed `frame`.
    ///
    /// The same question `wait_for_turn_frame` blocks on, asked once. The
    /// stepper uses it to re-confirm, under the frame gate, that the answer it
    /// waited for outside the gate is still true.
    fn frame_outstanding(&self, frame: SpectatorFrame) -> bool {
        lock_or_recover(&self.frame_delivery)
            .wait_remaining(frame, Instant::now())
            .is_some()
    }

    fn wait_for_turn_frame(&self, frame: SpectatorFrame) {
        let mut delivery = lock_or_recover(&self.frame_delivery);
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
    /// Seat the AIs: every major plays the deployment genome — `AdvancedAi`
    /// with the gene ledger applied — and minors and barbarians keep the
    /// cheaper baseline. A seat somebody is playing gets an agent too, so the
    /// world can be handed to it or watched from it.
    fn ai_fleet(game: &Game) -> Vec<Box<dyn Ai + Send>> {
        game.players
            .iter()
            .map(|p| -> Box<dyn Ai + Send> {
                if p.is_minor || p.is_barbarian {
                    Box::new(BasicAi::new())
                } else {
                    Box::new(AdvancedAi::new())
                }
            })
            .collect()
    }

    /// Name every seat a person is at. Sitting down to play does not make you
    /// one of the agents; the handle is minted for this game so the world can
    /// say who is playing, and nothing rates it.
    fn register_human_players(game: &Game) -> BTreeMap<usize, SeatPlayer> {
        let mut players = BTreeMap::new();
        for (index, seat) in game.human_seats.iter().copied().enumerate() {
            if game.players.get(seat).is_none() {
                continue;
            }
            let (name, username) = if index == 0 {
                ("player".to_string(), "Player".to_string())
            } else {
                (
                    format!("player{}", index + 1),
                    format!("Player {}", index + 1),
                )
            };
            players.insert(
                seat,
                SeatPlayer {
                    name,
                    username,
                    rated: false,
                },
            );
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
            tactics: params.tactics,
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
        game.mercy_rule = params.mercy_rule;
        game.required_victory_types = params.required_victory_types;
        // The hierarchical agent is the stock major-civilization default.
        // Minors/barbarians retain the cheaper baseline because they do not
        // need empire-level planning.
        let mut ais = Self::ai_fleet(&game);
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
        let human_players = Self::register_human_players(&game);
        let chronicle = ChronicleState::from_game(&game);
        Session {
            params,
            game,
            ais,
            spectator_paused: false,
            view_player: None,
            chronicle,
            resumed_next_game_params: None,
            autoplay_strategy: None,
            last_autoplay_request: None,
            human_players,
            journal,
            simultaneous_census: crate::simultaneous::SimultaneousCensus::default(),
            simultaneous_jobs: 1,
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
        params.mercy_rule = game.mercy_rule;
        params.required_victory_types = game.required_victory_types;
        params.tactics = game.tactics;
        params.teams = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.team)
            .collect();
        let next_game_params = (simulation_settings(&requested_next)
            != simulation_settings(&params))
        .then_some(requested_next);
        let mut ais = Self::ai_fleet(&game);
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
        // A save carries the world, not the person: whoever reloads it is a
        // new player again; a decided game names nobody.
        let human_players = if game.is_finished() {
            BTreeMap::new()
        } else {
            Self::register_human_players(&game)
        };
        Session {
            params,
            game,
            ais,
            spectator_paused: false,
            view_player: None,
            chronicle,
            resumed_next_game_params: next_game_params,
            autoplay_strategy: None,
            last_autoplay_request: None,
            human_players,
            journal,
            simultaneous_census: crate::simultaneous::SimultaneousCensus::default(),
            simultaneous_jobs: 1,
        }
    }

    pub(crate) fn set_view_player(&mut self, player: Option<usize>) -> Result<(), String> {
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

    /// Update the pause bit carried by spectator responses. The native and
    /// browser transports mirror this value in their own atomics/cells, but
    /// the session-side state transition is one protocol rule.
    pub(crate) fn set_spectator_paused(&mut self, paused: bool) {
        self.spectator_paused = paused;
    }

    /// Start a requested world, rejecting a delayed result-countdown request
    /// after the supervisor has already replaced the finished server.
    pub(crate) fn start_new_game(&mut self, request: &Value) -> Result<(), String> {
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
            if !self.game.is_finished()
                || self.game.seed != expected_seed
                || expected_instance != process_identity() as u64
            {
                return Err("finished game is no longer the active session".into());
            }
        } else if self.params.spectate
            && !self.game.is_finished()
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

    /// Whether this process may roll a finished world into its successor.
    ///
    /// A supervised exhibition deliberately leaves its terminal world in
    /// place for `spectator_supervisor.py`: that boundary fetches and builds
    /// canonical head before it replaces this process.  Starting here would
    /// let the old binary begin the next verification game first.
    fn may_start_automatic_successor(&self) -> bool {
        !self.params.supervised
    }

    /// Start the next world, from whatever setup the viewer queued for it.
    ///
    /// The queue is passed in rather than read from here: it is written by a
    /// setup control that must never wait on the simulation, so it lives
    /// beside the session rather than inside it.
    fn start_automatic_next_game(&mut self, queued: Option<Params>) -> bool {
        if !self.may_start_automatic_successor() {
            return false;
        }
        let next_seed = automatic_successor_seed(self.params.seed);
        let mut params = queued.unwrap_or_else(|| self.params.clone());
        params.seed = next_seed;
        *self = Session::new(params);
        true
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
        }
    }

    /// Give every met major both of its odds of winning this game: the ones
    /// it sat down with, and the ones it holds now. The odds themselves are
    /// [`crate::odds`]; every seat sits down at the same provisional prior,
    /// so the start odds are the difficulty bargain and the size of the
    /// table, which is exactly what they should be with nothing rating the
    /// seats.
    fn name_seat_odds(&self, o: &mut Value) {
        let g = &self.game;
        let odds = crate::odds::table(g, |_pid| 1500.0f64);
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

    /// The seat any question about the board is answered on behalf of, or
    /// `None` for the omniscient spectator. Read off the same two facts
    /// `state` branches on, and by the same rule, so a side question can never
    /// answer from a wider view than the observation the client is holding.
    pub fn viewing_seat(&self) -> Option<usize> {
        if self.params.spectate {
            self.view_player
        } else {
            Some(0)
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
                                player["research_boosted"] =
                                    json!(seat.research.as_ref().is_some_and(|tech| seat
                                        .boosted_techs
                                        .contains(&Name::new(tech))));
                                player["civic_boosted"] =
                                    json!(seat.civic.as_ref().is_some_and(|civic| seat
                                        .boosted_civics
                                        .contains(&Name::new(civic))));
                            }
                        }
                    }
                }
            }
            // Who is at each seat and its odds of winning from here.
            self.name_seat_odds(&mut o);
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
            o["victory_conditions"] = json!(self.game.effective_victory_conditions());
            o["mercy_rule"] = json!(self.game.mercy_rule);
            o["required_victory_types"] = json!(self.game.effective_required_victories());
            o["victories_won"] = json!(self.game.victories_won);
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
        self.name_seat_odds(&mut o);
        self.name_ai_players(&mut o);
        self.name_human_players(&mut o);
        o["spectate"] = json!(false);
        o["supervised"] = json!(self.params.supervised);
        o["view_player"] = json!(0);
        o["leader_pool"] = json!(self.game.leader_pool.id());
        o["turn_structure"] = json!(self.game.turn_structure.id());
        o["teams"] = json!(major_teams(&self.game));
        o["victory_conditions"] = json!(self.game.effective_victory_conditions());
        o["mercy_rule"] = json!(self.game.mercy_rule);
        o["required_victory_types"] = json!(self.game.effective_required_victories());
        o["victories_won"] = json!(self.game.victories_won);
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
        if g.is_finished() {
            return pid;
        }
        if g.finish_at_turn_limit() {
            return pid;
        }
        // A simultaneous game advances one whole game turn per step: every
        // seat plans against the same frozen world and the plans commit
        // through the ordinary rules. `simultaneous_jobs` is how many
        // planning workers the cycle may spread those seats across — one by
        // default, most of the machine while the exhibition runs at
        // Lightning — and the driver promises the same game at any count.
        // An aborted cycle is never retried: the driver has already said a
        // retry would replay the same turn forever, so the world sits still
        // instead.
        if g.turn_structure == TurnStructure::Simultaneous {
            if !self.simultaneous_census.aborted
                && !crate::simultaneous::step_cycle(
                    g,
                    &mut self.ais,
                    self.simultaneous_jobs,
                    &mut self.simultaneous_census,
                )
            {
                eprintln!(
                    "[server] simultaneous cycle aborted on turn {}: {}",
                    g.turn,
                    self.simultaneous_census.summary()
                );
            }
            g.finish_at_turn_limit();
            return pid;
        }
        self.ais[pid].take_turn(g, pid);
        if g.current == pid && !g.is_finished() {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        g.finish_at_turn_limit();
        // Every way of advancing the world funnels through here — the browser
        // stepping a batch, the headless pacer running an unattended
        // exhibition, autoplay — so this is the one place a result cannot be
        // missed.
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

    pub fn step_many(&mut self, count: usize) -> Vec<SpectatorStep> {
        let mut steps = Vec::new();
        for _ in 0..count.clamp(1, 12) {
            steps.push(self.spectator_step());
            if self.game.is_finished() {
                break;
            }
        }
        steps
    }

    /// Build the protocol response for a spectator step in one place. The
    /// native socket and the browser module differ only in what they do with
    /// the completed-frame signal after this method returns.
    pub(crate) fn spectator_step_response(&mut self, count: usize) -> (Value, bool) {
        let steps = self.step_many(count);
        let advanced = !steps.is_empty();
        let mut out = self.state();
        let visible_steps: Vec<_> = steps
            .iter()
            .filter(|step| self.view_player.is_none_or(|viewer| step.player == viewer))
            .collect();
        if let Some(step) = visible_steps.last() {
            out["stepped"] = json!(step.player);
            out["actions_taken"] = serde_json::to_value(&step.actions).unwrap_or(Value::Null);
        }
        out["step_batches"] = Value::Array(
            visible_steps
                .iter()
                .map(|step| {
                    json!({
                        "stepped": step.player,
                        "actions_taken": step.actions,
                        "world_events": if self.view_player.is_none() {
                            step.world_events.clone()
                        } else {
                            Vec::new()
                        },
                    })
                })
                .collect(),
        );
        (out, advanced)
    }

    /// Hand seat 0 to a named strategy, so auto-play runs *that* agent rather
    /// than whichever one the fleet happened to build for the seat.
    ///
    /// A name is matched against the built-in agents, by id or by the handle
    /// the picker shows. An unknown name is an error rather than a silent
    /// fallback: a player who picked a strategy and got a different one has
    /// been lied to.
    pub fn seat_strategy_at(&mut self, seat: usize, name: &str) -> Result<(), String> {
        if name.is_empty() || self.autoplay_strategy.as_deref() == Some(name) {
            return Ok(());
        }
        let seed = self.game.seed.wrapping_add(seat as u64);
        let id = BUILTIN_STRATEGIES
            .iter()
            .find(|(id, username)| *id == name || *username == name)
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("no strategy named {name}"))?;
        self.ais[seat] = crate::elo::builtin_send_ai(id, seed);
        // A newly seated agent joins the same record as the rest of the table;
        // without this the seat a player just handed over goes quiet.
        self.ais[seat].attach_journal(self.journal.handle());
        self.autoplay_strategy = Some(id.to_string());
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
        // Nobody has been handed this seat: the fleet built the default agent
        // there — "advanced", or the cheaper baseline for minors.
        let player = self.game.players.get(seat)?;
        Some(if player.is_minor || player.is_barbarian {
            "basic"
        } else {
            "advanced"
        })
    }

    /// Read the idempotency receipt for a completed auto-play batch.
    pub(crate) fn completed_autoplay(&self, request_id: &str) -> Option<usize> {
        self.last_autoplay_request
            .as_ref()
            .filter(|(completed, _)| completed == request_id)
            .map(|(_, played)| *played)
    }

    /// Remember the last auto-play batch so a retried request cannot simulate
    /// the same turns twice after a dropped response.
    pub(crate) fn remember_autoplay(&mut self, request_id: String, played: usize) {
        self.last_autoplay_request = Some((request_id, played));
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
            if self.game.is_finished() || !self.game.players[0].alive {
                break;
            }
            self.ais[0].take_turn(&mut self.game, 0);
            if self.game.current == 0 && !self.game.is_finished() {
                let _ = self.game.apply(0, &Action::EndTurn);
            }
            let g = &mut self.game;
            let mut guard = 0;
            while !g.is_finished()
                && g.current != 0
                && g.players[0].alive
                && guard < 2 * g.players.len()
            {
                let pid = g.current;
                self.ais[pid].take_turn(g, pid);
                if g.current == pid && !g.is_finished() {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
                guard += 1;
            }
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
            while !g.is_finished()
                && g.current != 0
                && g.players[0].alive
                && guard < 2 * g.players.len()
            {
                let pid = g.current;
                self.ais[pid].take_turn(g, pid);
                if g.current == pid && !g.is_finished() {
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
            while !g.is_finished()
                && g.current != 0
                && g.players[0].alive
                && guard < 2 * g.players.len()
            {
                let pid = g.current;
                self.ais[pid].take_turn(g, pid);
                if g.current == pid && !g.is_finished() {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
                guard += 1;
            }
        }
        None
    }
}

const APP_PALETTE_JS_TAG: &str = r#"<script src="/assets/app_palette.js"></script>"#;
const APP_JS_TAG: &str = r#"<script src="/assets/app.js"></script>"#;
const APP_SETUP_JS_TAG: &str = r#"<script src="/assets/app_setup.js"></script>"#;

/// Give each native server's application scripts a new URL.
///
/// A follower can replace the local HTTP process while the operator's Chrome
/// tab remains open.  `Cache-Control: no-store` normally makes a document
/// reload enough, but an instance-specific script URL means that even a tab
/// with an older cache entry cannot keep executing the former server's UI.
fn index_with_app_instance(page: &str, instance: u32) -> Vec<u8> {
    let palette_tag =
        format!(r#"<script src="/assets/app_palette.js?instance={instance}"></script>"#);
    let app_tag = format!(r#"<script src="/assets/app.js?instance={instance}"></script>"#);
    let setup_tag = format!(r#"<script src="/assets/app_setup.js?instance={instance}"></script>"#);
    page.replacen(APP_PALETTE_JS_TAG, &palette_tag, 1)
        .replacen(APP_JS_TAG, &app_tag, 1)
        .replacen(APP_SETUP_JS_TAG, &setup_tag, 1)
        .into_bytes()
}

fn index_html() -> Vec<u8> {
    for p in ["web/index.html"] {
        if let Ok(page) = std::fs::read_to_string(p) {
            return index_with_app_instance(&page, process_identity());
        }
    }
    index_with_app_instance(EMBEDDED_INDEX_HTML, process_identity())
}

fn app_js() -> Vec<u8> {
    for p in ["web/assets/app.js"] {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    EMBEDDED_APP_JS.as_bytes().to_vec()
}

fn app_palette_js() -> Vec<u8> {
    for p in ["web/assets/app_palette.js"] {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    EMBEDDED_APP_PALETTE_JS.as_bytes().to_vec()
}

fn app_setup_js() -> Vec<u8> {
    for p in ["web/assets/app_setup.js"] {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    EMBEDDED_APP_SETUP_JS.as_bytes().to_vec()
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

fn civ6_yield_icons() -> Vec<u8> {
    std::fs::read("web/assets/civ6-yield-icons.png")
        .unwrap_or_else(|_| EMBEDDED_CIV6_YIELD_ICONS.to_vec())
}

fn civ6_unit_flag_plates() -> Vec<u8> {
    std::fs::read("web/assets/civ6-unit-flag-plates.png")
        .unwrap_or_else(|_| EMBEDDED_CIV6_UNIT_FLAG_PLATES.to_vec())
}

fn civ6_city_banner_shields() -> Vec<u8> {
    std::fs::read("web/assets/civ6-city-banner-shields.png")
        .unwrap_or_else(|_| EMBEDDED_CIV6_CITY_BANNER_SHIELDS.to_vec())
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
    let save = crate::protocol::save_value(game).map_err(std::io::Error::other)?;
    let bytes = serde_json::to_vec(&save).map_err(std::io::Error::other)?;
    std::fs::write(&temporary, bytes)?;
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
            let name = path
                .file_name()?
                .to_str()?
                .strip_suffix(".save.json")?
                .to_string();
            let raw = std::fs::read(&path).ok()?;
            let value: Value = serde_json::from_slice(&raw).ok()?;
            let game = crate::protocol::game_from_save(value).ok()?;
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
                "finished": game.is_finished(),
                "draw": game.is_draw(),
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
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush())
        .is_ok()
}

fn respond_json(stream: &mut TcpStream, v: &Value) -> bool {
    // Preserve each endpoint's existing top-level shape while making the
    // protocol identity available to external clients. Insert into the
    // serialized object rather than cloning a potentially multi-megabyte
    // `/state` value just to add two fields.
    let mut body = v.to_string().into_bytes();
    if matches!(v, Value::Object(object) if !object.contains_key("protocol_version")) {
        let marker = if body.len() == 2 {
            format!(
                "\"protocol\":\"{}\",\"protocol_version\":{}",
                crate::protocol::PROTOCOL_NAME,
                crate::protocol::PROTOCOL_VERSION
            )
        } else {
            format!(
                "\"protocol\":\"{}\",\"protocol_version\":{},",
                crate::protocol::PROTOCOL_NAME,
                crate::protocol::PROTOCOL_VERSION
            )
        };
        if body.first() == Some(&b'{') {
            body.splice(1..1, marker.bytes());
        }
    }
    respond(stream, "200 OK", "application/json", &body)
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

/// The stock opening world: four majors playing themselves out on the Tiny
/// Lakes globe, its heat scattered rather than banded by latitude, one seat
/// at a time.
///
/// This is the one description of "the game nobody has decided anything
/// about yet". The browser build opens every civvis.ai visit on it (see
/// `wasm::opening_params`), `/rules` publishes it as `default_setup` so the
/// setup panel stamps its controls from here rather than from copies of
/// these values in the markup, and the lobby's own `selected` attributes are
/// held to it by test. Tweak this function to change what a first visit
/// loads into.
fn stock_opening_params(seed: u64) -> Params {
    let size = MapSize::for_players(4);
    let map_topology = MapTopology::Planet;
    let (width, height) = size.dimensions(map_topology);
    Params {
        num_players: 4,
        width,
        height,
        seed,
        base_ruleset: BaseRuleset::Civ6,
        start_era: 0,
        future_era: FutureEra::Classic,
        // The product is sequential, full stop: every world this server
        // rolls — watched or played — runs the one-seat-at-a-time regime,
        // and no request can pick anything else. The simultaneous driver
        // stays in the engine for research runs that launch with
        // `--turn-structure simultaneous`; it is simply not selectable
        // from here.
        turn_structure: TurnStructure::Sequential,
        map_script: MapScript::Lakes,
        map_topology,
        // Heat by noise instead of latitude: the world a first visit opens
        // on has no ice cap at either end, and its snow, desert and jungle
        // turn up wherever their own patch of noise puts them.
        map_poles: MapPoles::Randomized,
        game_speed: GameSpeed::Online,
        max_turns: GameSpeed::Online.turn_limit(),
        victory_conditions: VictoryConditions {
            science: true,
            culture: true,
            religious: true,
            diplomatic: true,
            domination: true,
            score: true,
        },
        // A new game plays to its natural end unless its owner explicitly
        // selects a Mercy Rule threshold in the setup panel.
        mercy_rule: None,
        required_victory_types: 1,
        // The stock world is not an arena, so this is carried rather than
        // read. It still ships the mode's own defaults, so that picking
        // Tactics in the lobby needs no other setting changed.
        tactics: TacticsRules::default(),
        num_city_states: size.default_city_states,
        spectate: true,
        difficulty: "prince".to_string(),
        speed: GameSpeed::Online.id().to_string(),
        teams: Vec::new(),
        leader_pool: LeaderPool::Civ6,
        civs: Vec::new(),
        supervised: false,
    }
}

/// [`stock_opening_params`] as the setup panel reads it. The seed is
/// removed: the stock world is a description, not a particular roll of it,
/// and a published seed would end up prefilled in the lobby's seed input.
pub(crate) fn default_setup_json() -> Value {
    let mut setup = simulation_settings(&stock_opening_params(0));
    setup
        .as_object_mut()
        .expect("simulation settings are an object")
        .remove("seed");
    setup
}

pub(crate) fn simulation_settings(params: &Params) -> Value {
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
        "mercy_rule": params.mercy_rule,
        "required_victory_types": params.required_victory_types,
    })
}

/// The agents a person can hand their seat to: the built-in agents, named by
/// id and by handle. Nothing rates them, so every row is provisional.
pub(crate) fn strategy_roster(_session: &Session) -> Value {
    json!(BUILTIN_STRATEGIES
        .iter()
        .map(|(name, username)| {
            json!({
                "name": name,
                "username": username,
                "label": name,
                "provisional": true,
            })
        })
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
    if let Some(v) = request["base_ruleset"]
        .as_str()
        .and_then(BaseRuleset::from_id)
    {
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
    // The turn structure is deliberately absent from this ladder: the
    // product is hard-committed to sequential turns, so a request cannot
    // select the regime at all. A world only plays simultaneous when the
    // server itself was launched into it (`--turn-structure` on a research
    // build), and the base params carry that through restarts below.
    if let Some(v) = request["map_script"].as_str().and_then(MapScript::from_id) {
        p.map_script = v;
        // A named battle carries the era and recommended clock of its own
        // briefing. Explicit lobby choices still win below, but a direct
        // client naming only `kadesh` or `mosul` gets the same opening the
        // browser gives it instead of inheriting the previous world.
        if let Some(scenario) = crate::historical_scenarios::by_script(v) {
            p.start_era = scenario.era_index;
            p.tactics.turn_limit = scenario.turns;
        }
        // `planet` used to name a world type; it now names a shape. A client
        // still asking for it by the old name means both halves of what it
        // used to mean, so the shape comes along with the type.
        if request["map_script"].as_str() == Some("planet") {
            p.map_topology = MapTopology::Planet;
        }
        // The Tactics planet has a deliberately custom globe size rather than
        // one of the ordinary Civ size ladder's rectangles. A browser sends
        // the dimensions explicitly, but direct clients still get the same
        // small world when they name the script alone.
        if v.is_planet_battlefield()
            && request["width"].as_i64().is_none()
            && request["height"].as_i64().is_none()
        {
            if let Some(size) = battlefield_sizes().iter().find(|size| size.script == v) {
                p.width = size.width;
                p.height = size.height;
                p.map_topology = size.topology;
            }
        }
    }
    if let Some(v) = request["map_topology"]
        .as_str()
        .and_then(MapTopology::from_id)
    {
        p.map_topology = v;
    }
    if let Some(v) = request["map_poles"].as_str().and_then(MapPoles::from_id) {
        p.map_poles = v;
    }
    // Heat was a boolean once — poles on or off. Only its `true` still names a
    // world that exists, and it names it plainly, so an old client sending it
    // gets the banded world it asked for. Its `false` asked for the retired
    // no-cold-end world, which nothing can build now, so that one falls
    // through to whatever the request otherwise settled on rather than being
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
    if let Some(v) = request["leader_pool"]
        .as_str()
        .and_then(LeaderPool::from_id)
    {
        p.leader_pool = v.available_or_default();
    }
    let selected_pool = p.leader_pool;
    p.civs.retain(|civ| {
        leader_roster::entry(civ)
            .is_some_and(|entry| entry.available && entry.pool == selected_pool)
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
    // A present key always wins, including an explicit null for "no mercy";
    // an absent key keeps the current setting, which starts off in every
    // stock setup. The band rejects thresholds a typo could smuggle in:
    // below 0.5 a "leader" is not even favoured.
    if let Some(value) = request.as_object().and_then(|o| o.get("mercy_rule")) {
        p.mercy_rule = value.as_f64().filter(|v| (0.5..1.0).contains(v));
    }
    if let Some(n) = request["required_victory_types"].as_u64() {
        p.required_victory_types = (n as usize).clamp(1, VictoryConditions::NAMES.len());
    }
    // The Tactics economy. Each figure is independent, so a lobby that moves
    // one leaves the rest at the stock grant, and every one of them is
    // clamped rather than trusted: these arrive from the same request body
    // as everything else above.
    if let Some(n) = request["tactics_cities"].as_u64() {
        p.tactics.cities = n.min(1) as u8;
    }
    for (key, field) in [
        ("tactics_production", 0),
        ("tactics_gold", 1),
        ("tactics_turns_per_tech", 2),
    ] {
        let Some(n) = request[key].as_u64() else {
            continue;
        };
        match field {
            0 => p.tactics.production = n.min(u64::from(TacticsRules::MAX_YIELD)) as u32,
            1 => p.tactics.gold = n.min(u64::from(TacticsRules::MAX_YIELD)) as u32,
            _ => {
                p.tactics.turns_per_tech = n.min(u64::from(TacticsRules::MAX_TURNS_PER_TECH)) as u32
            }
        }
    }
    if let Some(n) = request["tactics_best_of"].as_u64() {
        p.tactics.best_of = n.min(u64::from(TacticsRules::MAX_BEST_OF)) as u32;
    }
    if let Some(n) = request["tactics_turn_limit"].as_u64() {
        p.tactics.turn_limit = n.min(u64::from(u32::MAX)) as u32;
    }
    if let Some(on) = request["tactics_unique_units"].as_bool() {
        p.tactics.unique_units = on;
    }
    if let Some(on) = request["tactics_fog"].as_bool() {
        p.tactics.fog = on;
    }
    if let Some(on) = request["tactics_flag"].as_bool() {
        p.tactics.flag = on;
    }
    // Which era arms the battle: `random` re-rolls every battle of a series,
    // a rung's id fixes it, and `custom` reads the pool from `tactics_eras`.
    // The start-era contract holds here too — an id nobody has built, or a
    // custom pool that names no built rung, is refused rather than
    // substituted, so the previous setting stands and the client can see it
    // did.
    match request["tactics_era"].as_str() {
        Some("random") => p.tactics.era = TacticsEra::Random,
        Some("custom") => {
            if let Some(pool) = request["tactics_eras"].as_array() {
                let mask = pool
                    .iter()
                    .filter_map(|id| id.as_str())
                    .filter_map(start_era_from_id)
                    .fold(0u16, |mask, era| mask | 1 << era);
                if mask != 0 {
                    p.tactics.era = TacticsEra::Pool(mask);
                }
            }
        }
        Some(id) => {
            if let Some(era) = start_era_from_id(id) {
                p.tactics.era = TacticsEra::Fixed(era);
            }
        }
        None => {}
    }
    p.tactics = p.tactics.sanitized();
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
    // A Tactics map seats no city-states. The bounded battlefield is flat by
    // construction; the Tactics planet stays Planet so its two cities can be
    // placed on opposite sides of the globe. This lands after every override
    // above so the published next-game settings honestly describe the game
    // that will start; the engine enforces both again when the world is built.
    // A later request that moves the map back to a world restores the size
    // profile's city-states through its own `num_players` stamp.
    if p.map_script.is_battlefield() {
        p.map_topology = if p.map_script.is_planet_battlefield() {
            MapTopology::Planet
        } else {
            MapTopology::Flat
        };
        p.num_city_states = 0;
        // A scenario is drawn at the size of its own chart, so unlike an
        // arena its dimensions are not a setting: they land after any
        // explicit `width`/`height` above and overrule them. The engine
        // asserts the same thing when it reads the chart; this keeps the
        // published settings honest rather than letting the lobby advertise
        // a size the world will refuse.
        if let Some(size) = battlefield_sizes()
            .iter()
            .find(|size| size.script == p.map_script && size.script.is_scenario())
        {
            p.width = size.width;
            p.height = size.height;
        }
        // And its economy is the battle's, not the Tactics card's. Applied
        // here so `/setup` reports what will actually be played; `Game`
        // applies it again from `TacticsRules::for_script` when the world is
        // built, so a direct client cannot route around it.
        p.tactics = p.tactics.for_script(p.map_script);
        // And it keeps the selected battle clock rather than a civilization's
        // five hundred turns. An explicit general `turn_limit` in the same
        // request still wins, for direct clients that deliberately override
        // the Tactics menu.
        if requested_turn_limit(request).is_none() {
            p.max_turns = p.tactics.turn_limit;
        }
        // A battle is decided by the battle. Domination is the lane the last
        // army standing arrives through. The clock is not a score lane: if
        // both armies survive it, neither won and the battle is a draw.
        p.victory_conditions = VictoryConditions {
            science: false,
            culture: false,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        };
        p.required_victory_types = 1;
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
        let being_played_by_hand = !lock_or_recover(&sh.session).params.spectate;
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
                let s = lock_or_recover(&sh.session);
                spectator_frame(&s.game, sh.frame_sequence.load(Ordering::Relaxed))
            };
            sh.wait_for_turn_frame(current_frame);
            let gate = lock_or_recover(&sh.simulation_frame_gate);
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
            let mut s = lock_or_recover(&sh.session);
            if !s.params.spectate {
                // Seating can change between the check above and this one, so
                // this stays as the authority — but it releases the gate
                // before idling, for the same reason.
                drop(s);
                drop(simulation_frame_gate);
                std::thread::sleep(Duration::from_millis(300));
                continue;
            }
            if s.game.is_finished() {
                // A viewer who changed the countdown while this result was
                // on screen asked for the new length from that moment — a
                // new hold, so the viewer's clock re-anchors to it.
                if sh.finale_rearm.swap(false, Ordering::Relaxed) || over_since.is_none() {
                    over_since = Some(Instant::now());
                    sh.finale_hold.fetch_add(1, Ordering::Relaxed);
                }
                let t0 = over_since.unwrap_or_else(Instant::now);
                let left = final_countdown_ms(&sh).saturating_sub(t0.elapsed().as_millis() as u64);
                sh.restart_in.store(left, Ordering::Relaxed);
                if left == 0 && s.may_start_automatic_successor() {
                    // A Tactics match is a series, and the series outlives
                    // the battle: score this one and set the next one's sides
                    // before the world it was played on is replaced.
                    let queued = sh
                        .take_next_game_params()
                        .or_else(|| s.params.tactics.best_of.gt(&1).then(|| s.params.clone()));
                    let queued = sh.advance_match(&s.game, queued);
                    assert!(s.start_automatic_next_game(queued));
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
                sh.finale_rearm.store(false, Ordering::Relaxed);
                let step_started = Instant::now();
                let turn_before = s.game.turn;
                let finished_before = s.game.is_finished();
                s.simultaneous_jobs = simultaneous_jobs_for(pace);
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
                if s.game.is_finished() {
                    over_since = Some(Instant::now());
                    sh.finale_hold.fetch_add(1, Ordering::Relaxed);
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
        std::thread::sleep(Duration::from_millis(
            delay.saturating_sub(elapsed_ms).max(1),
        ));
    }
}

/// Attach exhibition metadata (restart countdown, pace, paused) to a state.
/// The world the setup panel is describing, normalized against the live one.
///
/// A free function because the queue has two homes: `Shared` on a server, and
/// a thread local in the wasm build, which has no threads to share between.
pub(crate) fn staged_next_game_params(base: &Params, request: &Value) -> Params {
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
        // Which hold this remainder belongs to; see `Shared::finale_hold`.
        o["restart_hold"] = json!(sh.finale_hold.load(Ordering::Relaxed));
    }
    // The match this battle belongs to, when the arena is being played as a
    // series. Absent for a single battle, so a viewer that finds it knows a
    // scoreline is worth showing.
    if let Some(series) = sh.match_series() {
        o["tactics_match"] = json!({
            "best_of": series.best_of,
            "wins_needed": series.wins_needed(),
            "played": series.played(),
            "wins": series.wins,
            "drawn": series.drawn,
            "scoreline": series.scoreline(),
            "winner": series.winner(),
            "decided": series.decided(),
        });
    }
    o["pace"] = json!(sh.pace_ms.load(Ordering::Relaxed));
    o["between_game_countdown_ms"] = json!(sh.between_game_countdown_ms.load(Ordering::Relaxed));
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

/// The ledger-held gene tags an operator's live verification arm forces on,
/// for `/gene-program`'s `armed` list: `CIVVIS_WITH` when exported — the same
/// deliberate override the game supervisor honours — otherwise the force
/// file the supervisor resolves before every batch. Display-grade on purpose:
/// the panel reports what the live seat is armed with on this machine, while
/// the supervisor remains the validator of the batch it actually launches,
/// and `routes::gene_program` drops any tag the ledger could not legally
/// seat. The browser build has no machine to read, so its list is empty.
fn armed_live_treatments() -> Vec<String> {
    let exported = std::env::var("CIVVIS_WITH")
        .ok()
        .filter(|list| !list.trim().is_empty());
    let raw = exported.or_else(|| {
        let path = std::env::var("CIVVIS_WITH_FILE")
            .ok()
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "{}/.civvis-live-force-on",
                    std::env::var("HOME").unwrap_or_default()
                )
            });
        std::fs::read_to_string(path).ok()
    });
    raw.map(|list| {
        list.split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

fn handle(stream: &mut TcpStream, sh: &Shared) {
    // A duplicated socket handle can fail under fd exhaustion or a raced
    // shutdown; the connection is not worth serving without a reader, and
    // `stream` itself is not yet in a state answerable with a response, so
    // this is dropped exactly as a failed first `read_line` is below.
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(clone);
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
    if let Err(error) = crate::protocol::validate_request(&parsed) {
        respond_json(stream, &json!({"error": error}));
        return;
    }

    match (method.as_str(), path.as_str()) {
        ("GET", path) if viewer_path(path) => {
            respond(stream, "200 OK", "text/html; charset=utf-8", &index_html());
        }
        ("GET", "/assets/app_palette.js") => {
            respond(
                stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                &app_palette_js(),
            );
        }
        ("GET", "/assets/app.js") => {
            respond(
                stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                &app_js(),
            );
        }
        ("GET", "/assets/app_setup.js") => {
            respond(
                stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                &app_setup_js(),
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
        ("GET", "/assets/civ6-yield-icons.png") => {
            respond(stream, "200 OK", "image/png", &civ6_yield_icons());
        }
        ("GET", "/assets/civ6-unit-flag-plates.png") => {
            respond(stream, "200 OK", "image/png", &civ6_unit_flag_plates());
        }
        ("GET", "/assets/civ6-city-banner-shields.png") => {
            respond(stream, "200 OK", "image/png", &civ6_city_banner_shields());
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
            let viewer = query_value(&request_target, "viewer")
                .unwrap_or("")
                .to_string();
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
                Some(lock_or_recover(&sh.simulation_frame_gate))
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
                    (Ok(turn), Some(Ok(seed)), Some("0") | None, Some(Ok(sequence))) => {
                        Some(SpectatorFrame {
                            seed,
                            turn,
                            finished: false,
                            sequence,
                        })
                    }
                    (Ok(turn), Some(Ok(seed)), Some("1"), Some(Ok(sequence))) => {
                        Some(SpectatorFrame {
                            seed,
                            turn,
                            finished: true,
                            sequence,
                        })
                    }
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
                let session = lock_or_recover(&sh.session);
                let frame =
                    spectator_frame(&session.game, sh.frame_sequence.load(Ordering::Relaxed));
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
            let session = lock_or_recover(&sh.session);
            let game = &session.game;
            respond_json(
                stream,
                &json!({
                    "turn": game.turn,
                    "winner": game.winner,
                    "finished": game.is_finished(),
                    "draw": game.is_draw(),
                    "victory_type": game.victory_type,
                    // How that result is denoted, so a supervisor reading the
                    // cheap endpoint can report a Mercy Rule ending by the
                    // lane it ended on rather than by the bare word. Same
                    // field, same meaning, as the one `/state` publishes.
                    "victory_label": game.victory_label(),
                    "spectate": session.params.spectate,
                    // Everything the spectator supervisor's poll loop reads,
                    // so watching for progress costs this small document
                    // instead of a multi-megabyte /state observation that
                    // also queues behind the simulation lock for longer.
                    "seed": game.seed,
                    "current": game.current,
                    "spectator_paused": session.spectator_paused,
                    "server_instance": process_identity(),
                    "decided": game.decided,
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
            if let Some(value) = parsed["between_game_countdown_ms"]
                .as_u64()
                .filter(|value| valid_between_game_countdown_ms(*value).is_some())
            {
                let before = sh.between_game_countdown_ms.swap(value, Ordering::Relaxed);
                // Changed while a result is being held: the new length is
                // counted from now. The stepper reads the flag on its next
                // pass and moves the hold's start.
                if before != value && sh.restart_in.load(Ordering::Relaxed) != u64::MAX {
                    sh.finale_rearm.store(true, Ordering::Relaxed);
                }
            }
            if let Some(v) = parsed["paused"].as_bool() {
                sh.paused.store(v, Ordering::Relaxed);
            }
            let mut session = lock_or_recover(&sh.session);
            let mut o = crate::routes::pace(&mut session, &parsed);
            drop(session);
            decorate(&mut o, sh);
            respond_json(stream, &o);
        }
        ("GET", "/save") => {
            let session = lock_or_recover(&sh.session);
            let save = crate::routes::save(&session);
            respond_json(stream, &save);
        }
        // The district adjacency calculator over the live game: every
        // buildable district for a city's civilization, every legal plot,
        // what each would earn there and the ledger saying why — foundations
        // counted as the districts they will become. A planning read for
        // spectator tools and the Civ 6 bridge's site advisor; read-only.
        // `?city=<id>` narrows to one city, the default is every city.
        ("GET", "/adjacency") => {
            let session = lock_or_recover(&sh.session);
            let only: Option<u32> =
                query_value(&request_target, "city").and_then(|city| city.parse().ok());
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
        // returns the first step. When that router has nothing — a column
        // packed into a defile, where the only way forward holds one of our
        // own units — the answer is a whole walk across it instead, see
        // `Game::pass_through_destination` and `docs/MOVEMENT.md`. Read-only
        // either way: the client sends `move_to` for whatever it is given,
        // which is the one action that may cross, so the engine remains the
        // authority on whether the walk is legal now.
        ("POST", "/route") => {
            let session = lock_or_recover(&sh.session);
            let answer = crate::routes::route_step(&session, &parsed);
            drop(session);
            respond_json(stream, &answer);
        }
        // What one unit affords, asked one unit at a time because the
        // observation cannot afford to carry it for all of them. The viewer
        // points at somebody else's unit and gets the two things standing
        // there decides: how far it could move this turn, and what it sees.
        ("POST", "/intel") => {
            let session = lock_or_recover(&sh.session);
            let answer = crate::routes::intel(&session, &parsed);
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
                respond_json(
                    stream,
                    &json!({"error": "a save name is letters, digits, - and _"}),
                );
                return;
            };
            let session = lock_or_recover(&sh.session);
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
            let Some(name) = parsed["name"].as_str() else {
                let mut session = lock_or_recover(&sh.session);
                let result = crate::routes::load_uploaded(&mut session, &parsed);
                let mut out = session.state();
                out["error"] = match result {
                    Ok(()) => Value::Null,
                    Err(error) => json!(error),
                };
                drop(session);
                decorate(&mut out, sh);
                respond_json(stream, &out);
                return;
            };
            let loaded: Result<Game, String> = save_path(name)
                .ok_or_else(|| "a save name is letters, digits, - and _".to_string())
                .and_then(|path| {
                    std::fs::read(&path).map_err(|error| format!("cannot read {name}: {error}"))
                })
                .and_then(|raw| {
                    serde_json::from_slice::<Value>(&raw)
                        .map_err(|error| format!("{name} is not JSON: {error}"))
                })
                .and_then(crate::protocol::game_from_save)
                .map_err(|error| format!("{name} is not a save: {error}"));
            let mut out = match loaded {
                Ok(game) => {
                    // A save records the mods it was played under. Loading it
                    // under a different set silently changes the rules
                    // mid-game, so refuse rather than pretend otherwise.
                    let active = crate::mods::active_names();
                    if game.mods != active {
                        let session = lock_or_recover(&sh.session);
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
                    let mut session = lock_or_recover(&sh.session);
                    let params = session.params.clone();
                    *session = Session::from_game(params, game);
                    sh.current_seed.store(session.game.seed, Ordering::Relaxed);
                    sh.adopt_live_params(&session.params);
                    if let Some(queued) = session.take_resumed_next_game_params() {
                        *lock_or_recover(&sh.next_game_params) = Some(queued);
                    }
                    let mut out = session.state();
                    out["error"] = Value::Null;
                    drop(session);
                    out
                }
                Err(error) => {
                    let session = lock_or_recover(&sh.session);
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
            let session = lock_or_recover(&sh.session);
            let answer = crate::routes::rules(&session, true);
            respond_json(stream, &answer);
        }
        ("GET", "/gene-program") => {
            let answer = crate::routes::gene_program(&armed_live_treatments());
            respond_json(stream, &answer);
        }
        // Hand your seat to one of our agents for a stretch of turns. `turns`
        // is a count or the string "all"; `strategy` names who plays, and is
        // remembered on the seat so a run continued in chunks stays one agent.
        ("POST", "/autoplay") => {
            let mut session = lock_or_recover(&sh.session);
            let outcome =
                match crate::routes::autoplay(&mut session, &parsed, Some(process_identity())) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        drop(session);
                        respond_json(stream, &error);
                        return;
                    }
                };
            let crate::routes::AutoplayOutcome {
                out,
                played,
                replayed,
            } = outcome;
            let mut out = out;
            drop(session);
            // One response carries one state, so every turn in this batch past
            // the first was played where nobody could see it. Recorded after
            // the session lock is released, and only on the path that actually
            // simulated: the retry arm above answers from
            // `last_autoplay_request` without playing anything, and counting
            // there would charge a dropped response twice.
            if !replayed {
                lock_or_recover(&sh.frame_delivery).turns_simulated_without_a_frame(played);
            }
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        ("GET", "/pedia") => {
            // Generated from the ruleset in play, mods included, so the GUI
            // reference never disagrees with the game it is attached to.
            let session = lock_or_recover(&sh.session);
            let answer = crate::routes::pedia(&session);
            drop(session);
            respond_json(stream, &answer);
        }
        ("POST", "/action") => {
            let mut session = lock_or_recover(&sh.session);
            let spectating = session.params.spectate;
            let outcome = crate::routes::action(&mut session, &parsed);
            let autosave = outcome.autosave_due(spectating);
            let mut out = outcome.out;
            // Civ 6 autosaves at the top of every turn, and the reason is the
            // same here: a single-player game that only exists in one
            // process's memory is one crash away from never having happened.
            // Spectated games are the supervisor's business, not this.
            if autosave {
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
            let mut session = lock_or_recover(&sh.session);
            let outcome = crate::routes::step(&mut session, &parsed);
            let mut out = outcome.out;
            let completed_frame = outcome.advanced.then(|| {
                let sequence = sh.frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                spectator_frame(&session.game, sequence)
            });
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
            let simulation_frame_gate = lock_or_recover(&sh.simulation_frame_gate);
            let mut session = lock_or_recover(&sh.session);
            let outcome = match crate::routes::play_on(&mut session, &parsed) {
                Ok(outcome) => outcome,
                Err(error) => {
                    drop(session);
                    drop(simulation_frame_gate);
                    respond_json(stream, &error);
                    return;
                }
            };
            if outcome.played_on {
                if let Some(paused) = outcome.requested_pause {
                    sh.paused.store(paused, Ordering::Relaxed);
                }
                // Clear this before taking the response snapshot, so it can
                // never promise both a paused continuation and a new world.
                sh.restart_in.store(u64::MAX, Ordering::Relaxed);
            }
            let mut out = outcome.out;
            drop(session);
            drop(simulation_frame_gate);
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        ("POST", "/view") => {
            let mut session = lock_or_recover(&sh.session);
            let out = crate::routes::view(&mut session, &parsed);
            drop(session);
            respond_json(stream, &out);
        }
        ("POST", "/spectator-status") => {
            let mut session = lock_or_recover(&sh.session);
            let answer = crate::routes::spectator_status(&mut session, &parsed);
            drop(session);
            respond_json(stream, &answer);
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
            let mut session = lock_or_recover(&sh.session);
            let result = crate::routes::new_game(&mut session, &parsed);
            if result.is_ok() {
                sh.current_seed.store(session.game.seed, Ordering::Relaxed);
                sh.adopt_live_params(&session.params);
                let paused = parsed["paused"]
                    .as_bool()
                    .unwrap_or_else(|| sh.paused.load(Ordering::Relaxed));
                sh.paused.store(paused, Ordering::Relaxed);
                session.set_spectator_paused(paused);
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
        match_series: Mutex::new(None),
        session: Mutex::new(session),
        pace_ms: AtomicU64::new(500), // half a second per turn by default
        between_game_countdown_ms: AtomicU64::new(DEFAULT_BETWEEN_GAME_COUNTDOWN_MS),
        finale_rearm: AtomicBool::new(false),
        finale_hold: AtomicU64::new(0),
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

#[cfg(test)]
mod tests;
