//! Glicko-2 strategy league: persistent skill ratings for high-level AI
//! strategies, with periodic selection so strong strategies breed offspring
//! and confidently weak ones retire.
//!
//! `civvis league` plays rating periods ("rounds") of multiplayer games
//! between named strategies — built-in agents plus parameterized AdvancedAi
//! variants (a `Weights` genome and an optional fixed victory lane). Each
//! round is one Glicko-2 rating period: every finished game decomposes into
//! pairwise results by placement, all games in the round update ratings at
//! once, and a strategy that sat out has only its uncertainty grow. Glicko-2
//! rather than Elo because the roster churns: a newborn strategy enters at
//! high rating deviation and converges quickly, while retirement decisions
//! can demand low deviation so nothing is culled on a small sample.
//!
//! Artifacts in the league dir: league.json (full roster + ratings, the one
//! source of truth), ratings.csv (per-round rating history), matches.csv
//! (every game played), and work/ (immutable distributed manifests/results).
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ai::{run_game, AdvancedAi, Ai, VictoryTarget, Weights};
use crate::game::Game;
use crate::rng::Rng;
use crate::setup::MapSize;

/// Glicko-2 works on an internal scale; ratings are stored and shown on the
/// familiar Elo-like scale (1500 start).
const SCALE: f64 = 173.7178;
const BASE_RATING: f64 = 1500.0;
const BASE_RD: f64 = 350.0;
const BASE_VOL: f64 = 0.06;
/// System constant: how much volatility can move per period. 0.5 is the
/// conservative end of Glickman's recommended 0.3..1.2.
const TAU: f64 = 0.5;
/// Selection uses a two-sided 95% confidence bound rather than treating a
/// noisy point estimate as settled skill.
const SELECTION_Z: f64 = 1.96;
/// A leader/civilization-specific rating starts at the player's global
/// strength, with extra uncertainty for the unmeasured combination effect.
const LEADER_EFFECT_RD: f64 = 200.0;
/// Retirement needs evidence: this many games and the deviation below this
/// bound, so an unlucky newcomer is never culled on noise.
const MIN_GAMES_TO_RETIRE: u32 = 20;
const MAX_RD_TO_RETIRE: f64 = 110.0;
/// Immutable work/result protocol. Bump this whenever a binary can no longer
/// execute a pending round exactly as an older binary would.
const WORK_SCHEMA_VERSION: u32 = 2;
/// A dead simulator's game becomes available again after this lease. Duplicate
/// execution is harmless because results have deterministic IDs and publish
/// with create-if-absent semantics.
const DEFAULT_LEASE_SECONDS: u64 = 60 * 60;
const LOCK_LEASE_SECONDS: u64 = 5 * 60;
const LOCK_RETRIES: usize = 2_400;
const WORKER_ACTIVE_SECONDS: u64 = 2 * 60;
const WORKER_DISCOVERY_MILLIS: u64 = 250;

/// How a seat materializes an `Ai`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StrategyKind {
    /// One of `elo::BUILTIN_AIS`.
    Builtin { ai: String },
    /// Parameterized AdvancedAi: a genome plus an optional fixed victory
    /// lane (stored as text; `VictoryTarget` parses it).
    Advanced {
        weights: Weights,
        target: Option<String>,
    },
}

/// Glicko state of one player using one leader/civilization combination.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CivRating {
    pub rating: f64,
    pub rd: f64,
    pub vol: f64,
    pub games: u32,
    pub wins: u32,
}

impl Default for CivRating {
    fn default() -> Self {
        CivRating {
            rating: BASE_RATING,
            rd: BASE_RD,
            vol: BASE_VOL,
            games: 0,
            wins: 0,
        }
    }
}

/// A leader/civilization table needs this many games before standings call it
/// settled. Its actual combination rating is still displayed after game one.
pub const CIV_ELO_MIN_GAMES: u32 = 5;

/// Online calibration audit for the rating system's pairwise predictions.
/// Sums, rather than rounded averages, keep the checkpoint lossless.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub comparisons: u64,
    pub brier_sum: f64,
    pub log_loss_sum: f64,
}

impl Calibration {
    fn record(&mut self, predicted: f64, actual: f64) {
        let predicted = predicted.clamp(1e-12, 1.0 - 1e-12);
        self.comparisons = self.comparisons.saturating_add(1);
        self.brier_sum += (predicted - actual) * (predicted - actual);
        self.log_loss_sum -= actual * predicted.ln() + (1.0 - actual) * (1.0 - predicted).ln();
    }

    pub fn brier(&self) -> f64 {
        self.brier_sum / self.comparisons.max(1) as f64
    }

    pub fn log_loss(&self) -> f64 {
        self.log_loss_sum / self.comparisons.max(1) as f64
    }

    fn since(&self, earlier: &Calibration) -> Calibration {
        Calibration {
            comparisons: self.comparisons.saturating_sub(earlier.comparisons),
            brier_sum: self.brier_sum - earlier.brier_sum,
            log_loss_sum: self.log_loss_sum - earlier.log_loss_sum,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strategy {
    pub name: String,
    /// Player handle shown on leaderboards, themed after the strategy it
    /// plays (a science bot reads as one). Backfilled on load for leagues
    /// saved before usernames existed.
    #[serde(default)]
    pub username: String,
    pub kind: StrategyKind,
    pub rating: f64,
    pub rd: f64,
    pub vol: f64,
    pub games: u32,
    pub wins: u32,
    /// Per-leader/per-civilization tables (leader -> civ -> Glicko state).
    /// Each named AI strategy is a player, matching the same identity model
    /// used for humans. Sparse combinations only update when actually played.
    #[serde(default)]
    pub leader_elo: BTreeMap<String, BTreeMap<String, CivRating>>,
    /// Migration source for league snapshots written before leaders were part
    /// of rating identity. It is consumed on load and never written again.
    #[serde(default, rename = "civ_elo", skip_serializing)]
    legacy_civ_elo: BTreeMap<String, CivRating>,
    pub born_round: u32,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub retired: bool,
    /// Anchors are never retired; keeping fixed reference agents in every
    /// era pins the rating scale so numbers stay comparable across rounds.
    #[serde(default)]
    pub anchor: bool,
    /// A person, registered when they sat down to play. They are rated here
    /// exactly like an agent — that is the whole point of one identity model
    /// — but they are not an entrant: `League::active` leaves them out, so
    /// nothing ever schedules, breeds, retires, or seats them. `kind` is the
    /// agent their seat falls back to if they hand it over to auto-play.
    #[serde(default)]
    pub human: bool,
}

impl Strategy {
    /// A fresh entrant at the base rating with no history behind it.
    pub fn new(name: &str, kind: StrategyKind, born_round: u32) -> Strategy {
        Strategy {
            name: name.to_string(),
            username: String::new(),
            kind,
            rating: BASE_RATING,
            rd: BASE_RD,
            vol: BASE_VOL,
            games: 0,
            wins: 0,
            leader_elo: BTreeMap::new(),
            legacy_civ_elo: BTreeMap::new(),
            born_round,
            parents: Vec::new(),
            retired: false,
            anchor: false,
            human: false,
        }
    }

    pub fn label(&self) -> String {
        if self.human {
            return "human".to_string();
        }
        match &self.kind {
            StrategyKind::Builtin { ai } => ai.clone(),
            StrategyKind::Advanced { target, .. } => match target {
                Some(lane) => format!("adv->{lane}"),
                None => "adv-genome".to_string(),
            },
        }
    }
}

/// Retired strategies stay in the roster (their history and lineage matter);
/// only active ones are scheduled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct League {
    pub round: u32,
    pub strategies: Vec<Strategy>,
    /// Predictive quality accumulated from games rated by this build. Older
    /// snapshots load with an empty audit and begin measuring forward.
    #[serde(default)]
    pub calibration: Calibration,
}

impl League {
    /// The entrants this league schedules, breeds, retires and seats — the
    /// agents still competing. Registered people are players in the same
    /// table and are rated by the same arithmetic, but they are never
    /// entrants: nothing may play a game *as* somebody who is not here.
    pub fn active(&self) -> Vec<usize> {
        (0..self.strategies.len())
            .filter(|i| !self.strategies[*i].retired && !self.strategies[*i].human)
            .collect()
    }

    /// Every registered person, in registration order.
    pub fn humans(&self) -> Vec<usize> {
        (0..self.strategies.len())
            .filter(|i| self.strategies[*i].human)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct LeagueCfg {
    /// Rating periods to play this invocation (state persists between runs).
    pub rounds: u32,
    pub games_per_round: u32,
    pub players_per_game: usize,
    pub width: i32,
    pub height: i32,
    pub max_turns: u32,
    pub num_city_states: usize,
    pub seed: u64,
    pub jobs: usize,
    pub dir: String,
    /// Breed and retire every this many rounds; 0 disables selection.
    pub evolve_every: u32,
    /// Active-roster cap that retirement trims back down to.
    pub max_pop: usize,
    pub verbose: bool,
    /// Stable name written into work leases. Set `CIVVIS_WORKER_ID` to a
    /// machine-unique value on a shared league; the process ID keeps two
    /// simulators on one machine distinct.
    pub worker_id: String,
    /// Abandoned game claims are automatically reclaimed after this long.
    pub lease_seconds: u64,
}

fn default_worker_id() -> String {
    let machine = std::env::var("CIVVIS_WORKER_ID")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "civvis-worker".to_string());
    format!("{machine}:{}", std::process::id())
}

/// Turns the shipped ruleset gives the league's game speed. Read from the rules
/// rather than pinned here, so a speed retune moves the league with it.
pub fn stock_turns() -> u32 {
    let rules = crate::rules::Rules::embedded();
    rules
        .speeds
        .get(&crate::game::default_speed())
        .map(|speed| speed.turns)
        .unwrap_or(500)
}

impl Default for LeagueCfg {
    fn default() -> Self {
        let size = MapSize::for_players(4);
        LeagueCfg {
            rounds: 10,
            games_per_round: 16,
            players_per_game: 4,
            width: size.width,
            height: size.height,
            // The stock budget for the speed the league plays, so games are
            // decided by winning rather than by whoever led on score when the
            // clock ran out. 150 turns made almost everything a score victory
            // and 250 was not enough either: over 9336 six-seat league games
            // at 250, 81.8% ended on the cap, no game ever ended on a natural
            // score victory, and domination and science never happened at all
            // -- so three of the seven bred niches were chasing outcomes that
            // could not occur. On six matched seeds a 250 cap ends 6 of 6
            // games on score at t251 while the stock budget ends all six
            // naturally between t288 and t369 (three diplomatic, three
            // religious) and changes who won in two of them, for 2.3x the
            // compute. See docs/EVAL.md.
            max_turns: stock_turns(),
            num_city_states: size.default_city_states,
            seed: 1,
            jobs: crate::parallel::default_jobs(),
            dir: "league".to_string(),
            evolve_every: 4,
            max_pop: 12,
            verbose: true,
            worker_id: default_worker_id(),
            lease_seconds: DEFAULT_LEASE_SECONDS,
        }
    }
}

/// The simulation settings travel with a round manifest. A worker always
/// executes these settings, not whatever flags happened to be supplied on
/// that machine, so every result in a rating period belongs to one experiment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WorkConfig {
    league_seed: u64,
    players_per_game: usize,
    width: i32,
    height: i32,
    max_turns: u32,
    num_city_states: usize,
    evolve_every: u32,
    max_pop: usize,
}

impl From<&LeagueCfg> for WorkConfig {
    fn from(cfg: &LeagueCfg) -> Self {
        Self {
            league_seed: cfg.seed,
            players_per_game: cfg.players_per_game,
            width: cfg.width,
            height: cfg.height,
            max_turns: cfg.max_turns,
            num_city_states: cfg.num_city_states,
            evolve_every: cfg.evolve_every,
            max_pop: cfg.max_pop,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WorkJob {
    id: String,
    seed: u64,
    /// Strategy names by starting seat. Names, rather than mutable roster
    /// indices, make result validation and audit files stable across evolution.
    table: Vec<String>,
    mirror_series: u32,
    rotation: u32,
}

/// Immutable description of one Glicko rating period. It snapshots both the
/// genomes and the schedule, allowing any compatible binary on any machine to
/// execute a job without touching mutable league state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct RoundManifest {
    schema_version: u32,
    engine: String,
    round: u32,
    created_unix: u64,
    config: WorkConfig,
    strategies: Vec<Strategy>,
    jobs: Vec<WorkJob>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct StoredOutcome {
    schema_version: u32,
    engine: String,
    worker: String,
    round: u32,
    job_id: String,
    /// Strategy names in finish order.
    placements: Vec<String>,
    leaders: Vec<String>,
    civs: Vec<String>,
    ranks: Vec<u32>,
    won: Vec<bool>,
    seed: u64,
    turn: u32,
    victory: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LeaseRecord {
    worker: String,
    process: u32,
    created_unix: u64,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn work_engine() -> String {
    format!(
        "civvis-{}-{}-league-work-{WORK_SCHEMA_VERSION}",
        env!("CARGO_PKG_VERSION"),
        option_env!("CIVVIS_COMMIT").unwrap_or("development")
    )
}

// ---------------------------------------------------------------------------
// Glicko-2 core (Glickman 2013, "Example of the Glicko-2 system").

#[derive(Clone, Copy)]
struct Glicko {
    mu: f64,
    phi: f64,
    sigma: f64,
}

fn to_internal(s: &Strategy) -> Glicko {
    Glicko {
        mu: (s.rating - BASE_RATING) / SCALE,
        phi: s.rd / SCALE,
        sigma: s.vol,
    }
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

fn expect(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

/// Symmetric pre-game expectation used to audit predictions. Glicko's update
/// equation conditions on the opponent's RD; a matchup forecast must include
/// uncertainty in both players or swapping their order would change it.
fn matchup_expectation(a: Glicko, b: Glicko) -> f64 {
    let combined_phi = (a.phi * a.phi + b.phi * b.phi).sqrt();
    1.0 / (1.0 + (-g(combined_phi) * (a.mu - b.mu)).exp())
}

fn leader_prior(global: Glicko) -> CivRating {
    let effect_phi = LEADER_EFFECT_RD / SCALE;
    CivRating {
        rating: BASE_RATING + SCALE * global.mu,
        rd: (SCALE * (global.phi * global.phi + effect_phi * effect_phi).sqrt()).min(BASE_RD),
        vol: global.sigma,
        games: 0,
        wins: 0,
    }
}

fn lower_confidence(s: &Strategy) -> f64 {
    s.rating - SELECTION_Z * s.rd
}

fn upper_confidence(s: &Strategy) -> f64 {
    s.rating + SELECTION_Z * s.rd
}

/// One rating period for one player. `results` are (opponent, score, weight)
/// with opponents at their PRE-period values, score 1/0.5/0. Pairwise results
/// from one multiplayer game sum to one effective observation instead of
/// pretending its correlated `n-1` comparisons were independent games.
/// Empty results = the player sat out: rating stays, uncertainty grows (capped
/// at the base RD so a long-idle strategy never looks more unknown than a
/// newborn).
fn rate(p: Glicko, results: &[(Glicko, f64, f64)]) -> Glicko {
    if results.is_empty() {
        let phi = (p.phi * p.phi + p.sigma * p.sigma).sqrt();
        return Glicko {
            phi: phi.min(BASE_RD / SCALE),
            ..p
        };
    }
    let mut v_inv = 0.0;
    let mut d_sum = 0.0;
    for (o, score, weight) in results {
        let gj = g(o.phi);
        let ej = expect(p.mu, o.mu, o.phi);
        v_inv += weight * gj * gj * ej * (1.0 - ej);
        d_sum += weight * gj * (score - ej);
    }
    let v = 1.0 / v_inv;
    let delta = v * d_sum;
    let (phi2, delta2) = (p.phi * p.phi, delta * delta);

    // New volatility: solve f(x)=0 by the paper's Illinois-style iteration.
    let a = (p.sigma * p.sigma).ln();
    let f = |x: f64| {
        let ex = x.exp();
        ex * (delta2 - phi2 - v - ex) / (2.0 * (phi2 + v + ex) * (phi2 + v + ex))
            - (x - a) / (TAU * TAU)
    };
    let mut lo = a;
    let mut hi = if delta2 > phi2 + v {
        (delta2 - phi2 - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * TAU) < 0.0 {
            k += 1.0;
        }
        a - k * TAU
    };
    let mut flo = f(lo);
    let mut fhi = f(hi);
    while (hi - lo).abs() > 1e-6 {
        let mid = lo + (lo - hi) * flo / (fhi - flo);
        let fmid = f(mid);
        if fmid * fhi <= 0.0 {
            lo = hi;
            flo = fhi;
        } else {
            flo /= 2.0;
        }
        hi = mid;
        fhi = fmid;
    }
    let sigma = (lo / 2.0).exp();
    let phi_star = (phi2 + sigma * sigma).sqrt();
    let phi = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    Glicko {
        mu: p.mu + phi * phi * d_sum,
        phi,
        sigma,
    }
}

fn apply_internal(s: &mut Strategy, g: Glicko) {
    s.rating = BASE_RATING + SCALE * g.mu;
    s.rd = SCALE * g.phi;
    s.vol = g.sigma;
}

// ---------------------------------------------------------------------------
// Seat -> Ai materialization.

fn make_ai(kind: &StrategyKind, seed: u64) -> Box<dyn Ai> {
    match kind {
        StrategyKind::Builtin { ai } => crate::elo::builtin_ai(ai, seed),
        StrategyKind::Advanced { weights, target } => {
            match target.as_deref().and_then(|t| t.parse::<VictoryTarget>().ok()) {
                Some(t) => Box::new(AdvancedAi::with_weights_and_target(weights.clone(), t)),
                None => Box::new(AdvancedAi::with_weights(weights.clone())),
            }
        }
    }
}

/// The genome a strategy contributes to breeding, if it has one. Built-in
/// advanced flavours breed from the weights they actually play with; agents
/// with no `Weights` genome (random, neural, ...) cannot be parents.
fn genome_of(kind: &StrategyKind) -> Option<Weights> {
    match kind {
        StrategyKind::Advanced { weights, .. } => Some(weights.clone()),
        StrategyKind::Builtin { ai } => match ai.as_str() {
            "advanced" | "advanced_v1" | "basic" => Some(Weights::default()),
            "advanced_evolved" | "evolved" => {
                Some(crate::evolve::load_champion("evolved").unwrap_or_default())
            }
            _ => None,
        },
    }
}

fn target_of(kind: &StrategyKind) -> Option<String> {
    match kind {
        StrategyKind::Advanced { target, .. } => target.clone(),
        StrategyKind::Builtin { .. } => None,
    }
}

/// Quality-diversity niche for an evolvable strategy. The six explicit
/// victory targets occupy 0..6 and untargeted AdvancedAi genomes occupy the
/// final generalist niche. Built-ins are rating anchors/benchmarks, not
/// members of the evolutionary archive.
const GENERALIST_NICHE: usize = VictoryTarget::ALL.len();
const EVOLUTION_NICHES: usize = GENERALIST_NICHE + 1;

fn evolution_niche(kind: &StrategyKind) -> Option<usize> {
    let StrategyKind::Advanced { target, .. } = kind else {
        return None;
    };
    let parsed = target
        .as_deref()
        .and_then(|lane| lane.parse::<VictoryTarget>().ok());
    Some(
        VictoryTarget::ALL
            .iter()
            .position(|lane| Some(*lane) == parsed)
            .unwrap_or(GENERALIST_NICHE),
    )
}

fn target_for_niche(niche: usize) -> Option<String> {
    VictoryTarget::ALL
        .get(niche)
        .map(|target| target.as_str().to_string())
}

fn conservative_order(league: &League, indices: &mut [usize]) {
    indices.sort_by(|a, b| {
        lower_confidence(&league.strategies[*b])
            .total_cmp(&lower_confidence(&league.strategies[*a]))
            .then_with(|| {
                league.strategies[*b]
                    .rating
                    .total_cmp(&league.strategies[*a].rating)
            })
            .then_with(|| league.strategies[*a].name.cmp(&league.strategies[*b].name))
    });
}

/// Pick the currently least-represented evolutionary niche. Ties rotate by
/// selection generation, so missing lanes are restored deterministically and
/// repeated selection does not always favour the enum's first lane.
fn next_niche(league: &League, cfg: &LeagueCfg, birth: usize) -> usize {
    let mut counts = [0usize; EVOLUTION_NICHES];
    for i in league.active() {
        if let Some(niche) = evolution_niche(&league.strategies[i].kind) {
            counts[niche] += 1;
        }
    }
    let least = *counts.iter().min().unwrap();
    let generation = league.round / cfg.evolve_every.max(1);
    let start = (generation as usize + birth) % EVOLUTION_NICHES;
    (0..EVOLUTION_NICHES)
        .map(|offset| (start + offset) % EVOLUTION_NICHES)
        .find(|niche| counts[*niche] == least)
        .unwrap()
}

/// Protect one conservatively best active genome in every represented niche.
/// This is the live quality-diversity archive: duplicates can still be culled,
/// but selection cannot silently erase an entire victory strategy again.
fn niche_elites(league: &League) -> std::collections::BTreeSet<usize> {
    let mut candidates: [Vec<usize>; EVOLUTION_NICHES] =
        std::array::from_fn(|_| Vec::new());
    for i in league.active() {
        if let Some(niche) = evolution_niche(&league.strategies[i].kind) {
            candidates[niche].push(i);
        }
    }
    candidates
        .iter_mut()
        .filter_map(|niche| {
            conservative_order(league, niche);
            niche.first().copied()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Player handles.

/// Handles per victory lane, so a username announces its strategy.
fn username_pool(lane: Option<&str>) -> &'static [&'static str] {
    match lane {
        Some("science") => &[
            "TechPriest", "LabRat", "BeakerBaron", "Eureka", "MoonshotMax", "QuantumLeap",
        ],
        Some("culture") => &[
            "CultureVulture", "OperaGhost", "PoetLaureate", "Wonderstruck", "TourismTycoon",
            "MuseTamer",
        ],
        Some("religious") => &[
            "ProphetMotive", "HolyRoller", "ZealotZed", "ApostlePaula", "FaithHealer",
            "TitheCollector",
        ],
        Some("diplomatic") => &[
            "SilverTongue", "Peacemonger", "Suzerain", "GrandBroker", "EnvoyElite",
            "CityStateFan",
        ],
        Some("domination") => &[
            "Warmonger", "SiegeLord", "BloodAndIron", "LegionLarry", "RaiderRex",
            "CapitalCollector",
        ],
        Some("score") => &[
            "PointHoarder", "ScoreKeeper", "TallyHo", "GrindKing", "MaxiMin", "NumbersNed",
        ],
        _ => &[
            "WildCard", "DarkHorse", "Maverick", "FreeSpirit", "Opportunist", "JackKnife",
        ],
    }
}

/// Founders keep fixed, recognizable handles across every league.
fn founder_username(name: &str) -> Option<&'static str> {
    Some(match name {
        "advanced" => "JackOfAllTrades",
        "basic" => "TrainingWheels",
        "advanced_v1" => "OldGuard",
        "evolved-champ" => "Darwin",
        "adv-science" => "TechPriest",
        "adv-culture" => "CultureVulture",
        "adv-religious" => "ProphetMotive",
        "adv-diplomatic" => "SilverTongue",
        "adv-domination" => "Warmonger",
        "adv-score" => "PointHoarder",
        _ => return None,
    })
}

fn unique_username(base: &str, taken: &std::collections::BTreeSet<String>) -> String {
    if !taken.contains(base) {
        return base.to_string();
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}{n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn lane_of(kind: &StrategyKind) -> Option<String> {
    target_of(kind)
}

/// Give every handle-less strategy a themed username. Founders get their
/// fixed handles; everyone else draws from their lane's pool, seeded by
/// their own name so backfill is deterministic whatever the roster order.
fn ensure_usernames(league: &mut League) {
    let mut taken: std::collections::BTreeSet<String> = league
        .strategies
        .iter()
        .map(|s| s.username.clone())
        .filter(|u| !u.is_empty())
        .collect();
    for i in 0..league.strategies.len() {
        if !league.strategies[i].username.is_empty() {
            continue;
        }
        let base = match founder_username(&league.strategies[i].name) {
            Some(handle) => handle.to_string(),
            None => {
                let seed = league.strategies[i]
                    .name
                    .bytes()
                    .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                        (h ^ b as u64).wrapping_mul(0x1_0000_0001_b3)
                    });
                let pool = username_pool(lane_of(&league.strategies[i].kind).as_deref());
                pool[Rng::new(seed).below(pool.len())].to_string()
            }
        };
        let handle = unique_username(&base, &taken);
        taken.insert(handle.clone());
        league.strategies[i].username = handle;
    }
}

/// Register a brand-new player in `league` and return their index.
///
/// Somebody who sits down to play is nobody who is already here. Handing them
/// an existing entrant would give them a rating they never earned and give
/// that entrant a result it never played for, so a seat a person takes always
/// gets a row of its own: a new handle, provisional at the base rating until
/// they finish a game. `kind` is only the agent the seat falls back to if
/// they hand it to auto-play; it is not a claim about how they play.
pub fn register_new_player(league: &mut League) -> usize {
    let names: BTreeSet<String> = league.strategies.iter().map(|s| s.name.clone()).collect();
    let handles: BTreeSet<String> = league
        .strategies
        .iter()
        .map(|s| s.username.clone())
        .collect();
    let kind = StrategyKind::Builtin {
        ai: "advanced".to_string(),
    };
    let mut player = Strategy::new(&unique_username("player", &names), kind, league.round);
    player.username = unique_username("Player", &handles);
    player.human = true;
    league.strategies.push(player);
    league.strategies.len() - 1
}

// ---------------------------------------------------------------------------
// Crash-safe shared-filesystem coordination.

fn serde_error(error: serde_json::Error) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, error)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> io::Result<T> {
    let raw = fs::read(path)?;
    serde_json::from_slice(&raw).map_err(serde_error)
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", unique_suffix()));
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        serde_json::to_writer_pretty(&mut file, value).map_err(serde_error)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        match fs::rename(&tmp, path) {
            Ok(()) => Ok(()),
            // Windows does not replace an existing destination with rename.
            // The state lock protects writers while a recoverable backup
            // preserves the previous checkpoint until the new one lands.
            Err(error)
                if path.exists()
                    && matches!(
                        error.kind(),
                        ErrorKind::AlreadyExists | ErrorKind::PermissionDenied
                    ) =>
            {
                let backup = path.with_file_name(format!(".{name}.{}.backup", unique_suffix()));
                fs::rename(path, &backup)?;
                match fs::rename(&tmp, path) {
                    Ok(()) => {
                        let _ = fs::remove_file(backup);
                        Ok(())
                    }
                    Err(error) => {
                        let _ = fs::rename(&backup, path);
                        Err(error)
                    }
                }
            }
            Err(error) => Err(error),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Publish immutable evidence exactly once. Hard-linking a complete temporary
/// file gives create-if-absent semantics; the fallback retains compatibility
/// with shared filesystems that do not support hard links.
fn publish_json_once<T: Serialize>(path: &Path, value: &T) -> io::Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        return Ok(false);
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("result.json");
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", unique_suffix()));
    let mut bytes = serde_json::to_vec_pretty(value).map_err(serde_error)?;
    bytes.push(b'\n');
    let mut temp = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    temp.write_all(&bytes)?;
    temp.sync_all()?;
    drop(temp);
    let published = match fs::hard_link(&tmp, path) {
        Ok(()) => true,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => false,
        // Claims normally guarantee one publisher. On filesystems without
        // hard links, rename the already-synced file only while the immutable
        // destination is absent; duplicate execution is deterministic.
        Err(_) if !path.exists() => {
            fs::rename(&tmp, path)?;
            true
        }
        Err(_) => false,
    };
    let _ = fs::remove_file(&tmp);
    Ok(published)
}

fn safe_fragment(value: &str) -> String {
    let mut out: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(48)
        .collect();
    if out.is_empty() {
        out.push_str("worker");
    }
    out
}

fn lease_is_stale(path: &Path, lease_seconds: u64) -> bool {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age.as_secs() >= lease_seconds.max(1))
}

fn displace_stale_lease(path: &Path, worker: &str, lease_seconds: u64) -> io::Result<bool> {
    if !lease_is_stale(path, lease_seconds) {
        return Ok(false);
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("lease");
    let stale = path.with_file_name(format!(
        ".{name}.stale-{}-{}",
        safe_fragment(worker),
        unique_suffix()
    ));
    match fs::rename(path, stale) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

struct LeagueLock {
    path: PathBuf,
    lease: LeaseRecord,
}

impl Drop for LeagueLock {
    fn drop(&mut self) {
        // Never delete a successor's lock after stale-lease recovery.
        if read_json::<LeaseRecord>(&self.path).ok().as_ref() == Some(&self.lease) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn acquire_league_lock(dir: &str, worker: &str) -> io::Result<LeagueLock> {
    let root = Path::new(dir);
    fs::create_dir_all(root)?;
    let path = root.join(".league.lock");
    let lease = LeaseRecord {
        worker: worker.to_string(),
        process: std::process::id(),
        created_unix: unix_now(),
    };
    for _ in 0..LOCK_RETRIES {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                serde_json::to_writer(&mut file, &lease).map_err(serde_error)?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                return Ok(LeagueLock {
                    path,
                    lease: lease.clone(),
                });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if displace_stale_lease(&path, worker, LOCK_LEASE_SECONDS)? {
                    continue;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::WouldBlock,
        format!("timed out waiting for league lock {}", path.display()),
    ))
}

fn round_dir(dir: &str, round: u32) -> PathBuf {
    Path::new(dir)
        .join("work")
        .join(format!("round-{round:08}"))
}

fn manifest_path(dir: &str, round: u32) -> PathBuf {
    round_dir(dir, round).join("manifest.json")
}

fn claim_path(dir: &str, round: u32, job: &str) -> PathBuf {
    round_dir(dir, round)
        .join("claims")
        .join(format!("{job}.json"))
}

fn result_path(dir: &str, round: u32, job: &str) -> PathBuf {
    round_dir(dir, round)
        .join("results")
        .join(format!("{job}.json"))
}

fn worker_path(dir: &str, worker: &str) -> PathBuf {
    Path::new(dir)
        .join("work")
        .join("workers")
        .join(format!("{}.json", safe_fragment(worker)))
}

struct WorkerPresence {
    path: PathBuf,
    lease: LeaseRecord,
}

impl Drop for WorkerPresence {
    fn drop(&mut self) {
        if read_json::<LeaseRecord>(&self.path).ok().as_ref() == Some(&self.lease) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl WorkerPresence {
    fn refresh(&mut self) -> io::Result<()> {
        let now = unix_now();
        if now.saturating_sub(self.lease.created_unix) < 5 {
            return Ok(());
        }
        self.lease.created_unix = now;
        atomic_write_json(&self.path, &self.lease)
    }
}

fn register_worker(cfg: &LeagueCfg) -> io::Result<WorkerPresence> {
    let lease = LeaseRecord {
        worker: cfg.worker_id.clone(),
        process: std::process::id(),
        created_unix: unix_now(),
    };
    let path = worker_path(&cfg.dir, &cfg.worker_id);
    atomic_write_json(&path, &lease)?;
    Ok(WorkerPresence { path, lease })
}

fn active_worker_count(dir: &str) -> usize {
    let path = Path::new(dir).join("work").join("workers");
    let Ok(entries) = fs::read_dir(path) else {
        return 1;
    };
    let workers: BTreeSet<String> = entries
        .flatten()
        .filter(|entry| !lease_is_stale(&entry.path(), WORKER_ACTIVE_SECONDS))
        .filter_map(|entry| read_json::<LeaseRecord>(&entry.path()).ok())
        .map(|lease| lease.worker)
        .collect();
    workers.len().max(1)
}

fn worker_round_contributions(cfg: &LeagueCfg, manifest: &RoundManifest) -> usize {
    manifest
        .jobs
        .iter()
        .filter(|job| {
            let completed = result_path(&cfg.dir, manifest.round, &job.id);
            if let Ok(result) = read_json::<StoredOutcome>(&completed) {
                return result.worker == cfg.worker_id;
            }
            // A result and its claim can coexist if the process dies after
            // durable publication but before best-effort claim cleanup. The
            // completed job belongs to its publisher and must not also count
            // as a leased contribution.
            if completed.exists() {
                return false;
            }
            let path = claim_path(&cfg.dir, manifest.round, &job.id);
            let Ok(lease) = read_json::<LeaseRecord>(&path) else {
                return false;
            };
            // Expired work is no longer a contribution. In particular, a
            // replacement process commonly has the same stable worker ID as
            // the process that died. Counting its abandoned claims can fill
            // the fair-share quota and make `claim_jobs` return before
            // `try_claim_job` ever gets the chance to reclaim them.
            lease.worker == cfg.worker_id && !lease_is_stale(&path, cfg.lease_seconds)
        })
        .count()
}

fn validate_manifest(manifest: &RoundManifest, league: &League) -> io::Result<()> {
    if manifest.schema_version != WORK_SCHEMA_VERSION
        || manifest.engine != work_engine()
        || manifest.round != league.round
        || manifest.strategies != league.strategies
        || manifest.config.players_per_game < 2
        || manifest.config.width <= 0
        || manifest.config.height <= 0
        || manifest.config.max_turns == 0
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("incompatible round {} manifest", league.round),
        ));
    }
    let names: BTreeSet<&str> = manifest
        .strategies
        .iter()
        .map(|strategy| strategy.name.as_str())
        .collect();
    if names.len() != manifest.strategies.len() || manifest.jobs.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "manifest has duplicate strategies or no jobs",
        ));
    }
    let mut ids = BTreeSet::new();
    for job in &manifest.jobs {
        if !ids.insert(&job.id)
            || safe_fragment(&job.id) != job.id
            || job.table.len() != manifest.config.players_per_game
            || job.table.iter().any(|name| !names.contains(name.as_str()))
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid manifest job {}", job.id),
            ));
        }
    }
    let players = manifest.config.players_per_game;
    if manifest.jobs.len() % players != 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "manifest ends with a partial mirror series",
        ));
    }
    for (series_index, series) in manifest.jobs.chunks(players).enumerate() {
        let base = &series[0];
        for (rotation, job) in series.iter().enumerate() {
            let rotated_correctly = (0..players)
                .all(|seat| job.table[seat] == base.table[(seat + rotation) % players]);
            if job.mirror_series != series_index as u32
                || job.rotation != rotation as u32
                || job.seed != base.seed
                || !rotated_correctly
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("broken mirrored series {series_index}"),
                ));
            }
        }
    }
    Ok(())
}

/// Must be called with the league lock held.
fn load_or_create_manifest(league: &League, cfg: &LeagueCfg) -> io::Result<RoundManifest> {
    let path = manifest_path(&cfg.dir, league.round);
    let manifest = match read_json(&path) {
        Ok(manifest) => manifest,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let manifest = build_manifest(league, cfg);
            atomic_write_json(&path, &manifest)?;
            manifest
        }
        Err(error) => return Err(error),
    };
    validate_manifest(&manifest, league)?;
    Ok(manifest)
}

#[derive(Clone)]
struct ClaimedJob {
    job: WorkJob,
    lease: LeaseRecord,
}

fn try_claim_job(
    cfg: &LeagueCfg,
    manifest: &RoundManifest,
    job: &WorkJob,
) -> io::Result<Option<ClaimedJob>> {
    if result_path(&cfg.dir, manifest.round, &job.id).exists() {
        return Ok(None);
    }
    let path = claim_path(&cfg.dir, manifest.round, &job.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    for _ in 0..2 {
        let lease = LeaseRecord {
            worker: cfg.worker_id.clone(),
            process: std::process::id(),
            created_unix: unix_now(),
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                serde_json::to_writer(&mut file, &lease).map_err(serde_error)?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                return Ok(Some(ClaimedJob {
                    job: job.clone(),
                    lease,
                }));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if !displace_stale_lease(&path, &cfg.worker_id, cfg.lease_seconds)? {
                    return Ok(None);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

fn claim_jobs(cfg: &LeagueCfg, manifest: &RoundManifest) -> io::Result<Vec<ClaimedJob>> {
    let workers = active_worker_count(&cfg.dir);
    let fair_share = manifest.jobs.len().div_ceil(workers);
    let already_contributing = worker_round_contributions(cfg, manifest);
    let claim_limit = fair_share
        .saturating_sub(already_contributing)
        .min(cfg.jobs.max(1));
    if claim_limit == 0 {
        return Ok(Vec::new());
    }
    let mut claimed = Vec::new();
    for job in &manifest.jobs {
        if let Some(job) = try_claim_job(cfg, manifest, job)? {
            claimed.push(job);
            if claimed.len() >= claim_limit {
                break;
            }
        }
    }
    Ok(claimed)
}

fn release_claim(cfg: &LeagueCfg, round: u32, claimed: &ClaimedJob) {
    let path = claim_path(&cfg.dir, round, &claimed.job.id);
    if read_json::<LeaseRecord>(&path).ok().as_ref() == Some(&claimed.lease) {
        let _ = fs::remove_file(path);
    }
}

fn validate_result(
    manifest: &RoundManifest,
    job: &WorkJob,
    result: &StoredOutcome,
) -> io::Result<()> {
    let count = manifest.config.players_per_game;
    let mut expected = job.table.clone();
    let mut actual = result.placements.clone();
    expected.sort();
    actual.sort();
    let valid_ranks = result.ranks.first() == Some(&0)
        && result.ranks.iter().all(|rank| *rank < count as u32)
        && result
            .ranks
            .iter()
            .enumerate()
            .skip(1)
            .all(|(place, rank)| *rank == result.ranks[place - 1] || *rank == place as u32);
    let winners = result.won.iter().filter(|won| **won).count();
    if result.schema_version != manifest.schema_version
        || result.engine != manifest.engine
        || result.worker.trim().is_empty()
        || result.round != manifest.round
        || result.job_id != job.id
        || result.seed != job.seed
        || result.placements.len() != count
        || result.leaders.len() != count
        || result.civs.len() != count
        || result.ranks.len() != count
        || result.won.len() != count
        || result.civs.iter().any(|civ| civ.trim().is_empty())
        || result.leaders.iter().any(|leader| leader.trim().is_empty())
        || expected != actual
        || !valid_ranks
        || winners > 1
        || (winners == 1 && !result.won[0])
        || (winners == 1 && result.ranks.iter().skip(1).any(|rank| *rank == 0))
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("invalid result for job {}", job.id),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// League lifecycle.

/// Founding roster: anchor reference agents, the six fixed victory lanes
/// (the "particular higher-level strategies" the league exists to compare),
/// and the GA champion if one has been evolved on this machine.
fn seed_league(dir: &str) -> League {
    let mut strategies = Vec::new();
    let mut builtin = |name: &str, ai: &str, anchor: bool| {
        let mut s = Strategy::new(
            name,
            StrategyKind::Builtin { ai: ai.to_string() },
            0,
        );
        s.anchor = anchor;
        strategies.push(s);
    };
    builtin("advanced", "advanced", true);
    builtin("basic", "basic", true);
    builtin("advanced_v1", "advanced_v1", false);
    for lane in VictoryTarget::ALL {
        strategies.push(Strategy::new(
            &format!("adv-{}", lane.as_str()),
            StrategyKind::Advanced {
                weights: Weights::default(),
                target: Some(lane.as_str().to_string()),
            },
            0,
        ));
    }
    if let Some(w) = crate::evolve::load_champion("evolved") {
        strategies.push(Strategy::new(
            "evolved-champ",
            StrategyKind::Advanced {
                weights: w,
                target: None,
            },
            0,
        ));
    }
    let mut league = League {
        round: 0,
        strategies,
        calibration: Calibration::default(),
    };
    ensure_usernames(&mut league);
    save_league(dir, &league);
    league
}

pub fn load_league(dir: &str) -> Option<League> {
    let raw = fs::read_to_string(Path::new(dir).join("league.json")).ok()?;
    let mut league: League = serde_json::from_str(&raw).ok()?;
    migrate_legacy_leader_ratings(&mut league);
    ensure_usernames(&mut league);
    Some(league)
}

fn default_leader(civilization: &str) -> String {
    crate::elo::leader_for_civilization(civilization)
}

fn migrate_legacy_leader_ratings(league: &mut League) {
    for player in &mut league.strategies {
        for (civilization, rating) in std::mem::take(&mut player.legacy_civ_elo) {
            let leader = default_leader(&civilization);
            player
                .leader_elo
                .entry(leader)
                .or_default()
                .entry(civilization)
                .or_insert(rating);
        }
    }
}

fn combination_rating<'a>(
    player: &'a Strategy,
    leader: &str,
    civilization: &str,
) -> Option<&'a CivRating> {
    player.leader_elo.get(leader)?.get(civilization)
}

/// Write via a temp file + rename so a crash mid-write cannot lose the roster.
pub fn save_league(dir: &str, league: &League) {
    let _ = save_league_checked(dir, league);
}

fn save_league_checked(dir: &str, league: &League) -> io::Result<()> {
    atomic_write_json(&Path::new(dir).join("league.json"), league)
}

/// Per-round RNG derived from (seed, round) so a resumed league plays the
/// same schedule it would have played in one continuous run.
fn round_rng(seed: u64, round: u32) -> Rng {
    Rng::new(seed ^ 0x1EA6_0000 ^ (round as u64).wrapping_mul(0x9E37_79B9))
}

/// A round's tables: shuffle the active roster and deal it into tables of
/// `players_per_game`, repeating passes until `games_per_round` tables exist.
/// Everyone plays a near-equal amount and mixing is uniform; with rosters
/// this small (<=~16) proximity matchmaking would only slow convergence.
fn schedule_tables(
    active: &[usize],
    games: usize,
    players_per_game: usize,
    rng: &mut Rng,
) -> Vec<Vec<usize>> {
    assert!(!active.is_empty());
    let mut tables = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    while tables.len() < games {
        if order.len() < players_per_game {
            let mut pass = active.to_vec();
            for i in (1..pass.len()).rev() {
                pass.swap(i, rng.below(i + 1));
            }
            order.extend(pass);
        }
        let take = players_per_game.min(order.len());
        let mut table: Vec<usize> = order.drain(..take).collect();
        while table.len() < players_per_game {
            table.push(active[rng.below(active.len())]);
        }
        // A table of clones rates nobody; force a second strategy in.
        if active.len() > 1 && table.iter().all(|s| *s == table[0]) {
            let others: Vec<usize> = active.iter().copied().filter(|s| *s != table[0]).collect();
            let seat = rng.below(table.len());
            table[seat] = others[rng.below(others.len())];
        }
        tables.push(table);
    }
    tables
}

#[cfg(test)]
fn schedule(active: &[usize], cfg: &LeagueCfg, rng: &mut Rng) -> Vec<Vec<usize>> {
    schedule_tables(
        active,
        cfg.games_per_round as usize,
        cfg.players_per_game,
        rng,
    )
}

/// Construct complete mirrored series. A base matchup repeats on the exact
/// same map while its strategies rotate through every starting seat/civ. This
/// removes much of the civilization, spawn, and first-move noise from strategy
/// comparisons. The requested game count rounds up to a complete series.
fn build_manifest(league: &League, cfg: &LeagueCfg) -> RoundManifest {
    let players = cfg.players_per_game;
    let series = (cfg.games_per_round as usize).div_ceil(players).max(1);
    let mut rng = round_rng(cfg.seed, league.round);
    let bases = schedule_tables(&league.active(), series, players, &mut rng);
    let mut jobs = Vec::with_capacity(series * players);
    for (series_index, table) in bases.into_iter().enumerate() {
        let seed = cfg
            .seed
            .wrapping_mul(1_000_003)
            .wrapping_add(league.round as u64 * 4096 + series_index as u64);
        for rotation in 0..players {
            let rotated = (0..players)
                .map(|seat| {
                    league.strategies[table[(seat + rotation) % players]]
                        .name
                        .clone()
                })
                .collect();
            jobs.push(WorkJob {
                id: format!("r{:08}-s{:06}-p{:03}", league.round, series_index, rotation),
                seed,
                table: rotated,
                mirror_series: series_index as u32,
                rotation: rotation as u32,
            });
        }
    }
    RoundManifest {
        schema_version: WORK_SCHEMA_VERSION,
        engine: work_engine(),
        round: league.round,
        created_unix: unix_now(),
        config: WorkConfig::from(cfg),
        strategies: league.strategies.clone(),
        jobs,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Outcome {
    /// Strategy indices, winner first then by score.
    placements: Vec<usize>,
    /// Leader each placement used, aligned with `placements`.
    leaders: Vec<String>,
    /// Civ each placement played, aligned with `placements`.
    civs: Vec<String>,
    /// Competition ranks aligned with `placements`; equal scores share a rank
    /// unless one seat is the engine-declared winner.
    ranks: Vec<u32>,
    /// Whether each placement won an engine-declared victory. A score tie in
    /// a game with no winner is a draw, not a win for the lower seat id.
    won: Vec<bool>,
    seed: u64,
    turn: u32,
    victory: String,
}

fn play_job(manifest: &RoundManifest, job: &WorkJob, worker: &str) -> StoredOutcome {
    let cfg = &manifest.config;
    let mut game = Game::new(
        cfg.players_per_game,
        cfg.width,
        cfg.height,
        job.seed,
        cfg.max_turns,
        cfg.num_city_states,
    );
    let mut ais: Vec<Box<dyn Ai>> = game
        .players
        .iter()
        .map(|player| {
            if player.id < cfg.players_per_game {
                let name = &job.table[player.id];
                let strategy = manifest
                    .strategies
                    .iter()
                    .find(|strategy| &strategy.name == name)
                    .expect("manifest job references a missing strategy");
                make_ai(&strategy.kind, job.seed.wrapping_add(player.id as u64))
            } else {
                crate::elo::builtin_ai("basic", job.seed.wrapping_add(player.id as u64))
            }
        })
        .collect();
    run_game(&mut game, &mut ais);

    let mut ranked: Vec<usize> = (0..cfg.players_per_game).collect();
    ranked.sort_by_key(|pid| (game.winner != Some(*pid), -game.score(*pid), *pid));
    let mut ranks = Vec::with_capacity(ranked.len());
    for (place, pid) in ranked.iter().copied().enumerate() {
        let same_as_previous = place > 0
            && (game.winner == Some(pid)) == (game.winner == Some(ranked[place - 1]))
            && game.score(pid) == game.score(ranked[place - 1]);
        ranks.push(if same_as_previous {
            ranks[place - 1]
        } else {
            place as u32
        });
    }
    StoredOutcome {
        schema_version: WORK_SCHEMA_VERSION,
        engine: manifest.engine.clone(),
        worker: worker.to_string(),
        round: manifest.round,
        job_id: job.id.clone(),
        placements: ranked.iter().map(|pid| job.table[*pid].clone()).collect(),
        leaders: ranked
            .iter()
            .map(|pid| {
                let civilization = &game.players[*pid].civ;
                game.rules
                    .civs
                    .get(civilization)
                    .map(|spec| spec.leader.clone())
                    .unwrap_or_else(|| civilization.clone())
            })
            .collect(),
        civs: ranked
            .iter()
            .map(|pid| game.players[*pid].civ.clone())
            .collect(),
        ranks,
        won: ranked.iter().map(|pid| game.winner == Some(*pid)).collect(),
        seed: job.seed,
        turn: game.turn,
        victory: game.victory_type.clone().unwrap_or_default(),
    }
}

/// One Glicko-2 rating period: every game becomes pairwise results against
/// opponents' pre-round ratings, then all active strategies update at once.
///
/// `age_idle` decides what happens to strategies that sat the period out. A
/// league round schedules the whole roster, so anyone missing really did idle
/// and their deviation should grow. A single recorded game is a period only
/// six seats could enter, so ageing the rest would pin the roster at maximum
/// uncertainty within an afternoon — the same reason combination tables are sparse.
fn apply_round(league: &mut League, outcomes: &[Outcome], age_idle: bool) {
    let pre: Vec<Glicko> = league.strategies.iter().map(to_internal).collect();
    let mut results: BTreeMap<usize, Vec<(Glicko, f64, f64)>> = BTreeMap::new();
    let mut combination_pre = BTreeMap::<(usize, String, String), Glicko>::new();
    for outcome in outcomes {
        for (place, player) in outcome.placements.iter().copied().enumerate() {
            let key = (
                player,
                outcome.leaders[place].clone(),
                outcome.civs[place].clone(),
            );
            combination_pre.entry(key).or_insert_with(|| {
                combination_rating(
                    &league.strategies[player],
                    &outcome.leaders[place],
                    &outcome.civs[place],
                )
                .map(|rating| Glicko {
                    mu: (rating.rating - BASE_RATING) / SCALE,
                    phi: rating.rd / SCALE,
                    sigma: rating.vol,
                })
                .unwrap_or_else(|| {
                    let prior = leader_prior(pre[player]);
                    Glicko {
                        mu: (prior.rating - BASE_RATING) / SCALE,
                        phi: prior.rd / SCALE,
                        sigma: prior.vol,
                    }
                })
            });
        }
    }
    let mut combination_results =
        BTreeMap::<(usize, String, String), Vec<(Glicko, f64, f64)>>::new();
    for outcome in outcomes {
        let p = &outcome.placements;
        let comparison_weight = 1.0 / p.len().saturating_sub(1).max(1) as f64;
        for i in 0..p.len() {
            for j in (i + 1)..p.len() {
                if p[i] == p[j] {
                    continue; // a strategy cannot rate itself
                }
                let score_i = match outcome.ranks[i].cmp(&outcome.ranks[j]) {
                    std::cmp::Ordering::Less => 1.0,
                    std::cmp::Ordering::Equal => 0.5,
                    std::cmp::Ordering::Greater => 0.0,
                };
                results
                    .entry(p[i])
                    .or_default()
                    .push((pre[p[j]], score_i, comparison_weight));
                results.entry(p[j]).or_default().push((
                    pre[p[i]],
                    1.0 - score_i,
                    comparison_weight,
                ));
                let key_i = (p[i], outcome.leaders[i].clone(), outcome.civs[i].clone());
                let key_j = (p[j], outcome.leaders[j].clone(), outcome.civs[j].clone());
                combination_results.entry(key_i.clone()).or_default().push((
                    combination_pre[&key_j],
                    score_i,
                    comparison_weight,
                ));
                combination_results.entry(key_j.clone()).or_default().push((
                    combination_pre[&key_i],
                    1.0 - score_i,
                    comparison_weight,
                ));
                league.calibration.record(
                    matchup_expectation(combination_pre[&key_i], combination_pre[&key_j]),
                    score_i,
                );
            }
        }
        for (rank, s) in p.iter().enumerate() {
            let strategy = &mut league.strategies[*s];
            strategy.games += 1;
            if outcome.won[rank] {
                strategy.wins += 1;
            }
            let prior = leader_prior(pre[*s]);
            let on_combination = strategy
                .leader_elo
                .entry(outcome.leaders[rank].clone())
                .or_default()
                .entry(outcome.civs[rank].clone())
                .or_insert(prior);
            on_combination.games += 1;
            if outcome.won[rank] {
                on_combination.wins += 1;
            }
        }
    }
    let combination_updates: Vec<((usize, String, String), Glicko)> = combination_results
        .into_iter()
        .map(|(key, res)| {
            let state = combination_pre[&key];
            (key, rate(state, &res))
        })
        .collect();
    for ((player, leader, civilization), updated) in combination_updates {
        let rating = league.strategies[player]
            .leader_elo
            .get_mut(&leader)
            .and_then(|civilizations| civilizations.get_mut(&civilization))
            .unwrap();
        rating.rating = BASE_RATING + SCALE * updated.mu;
        rating.rd = SCALE * updated.phi;
        rating.vol = updated.sigma;
    }
    let empty = Vec::new();
    for i in 0..league.strategies.len() {
        if league.strategies[i].retired {
            continue;
        }
        let played = results.get(&i);
        if played.is_none() && !age_idle {
            continue;
        }
        let updated = rate(pre[i], played.unwrap_or(&empty));
        apply_internal(&mut league.strategies[i], updated);
    }
}

/// Rate one finished game as its own rating period and persist it, so a
/// server playing rated seats actually moves the table instead of showing a
/// snapshot forever.
///
/// `placements` is (player/strategy id, civilization) ordered winner first,
/// then by score. The active ruleset resolves the matching leader. The roster
/// is re-read from `dir` and seats are resolved by stable strategy id
/// rather than by the index the caller seated from: a live server holds its
/// league in memory for the length of a game, and writing that stale copy
/// back would undo any result recorded in the meantime. Returns the updated
/// league, or `None` if the roster is unreadable or no longer holds every
/// name (a retired or renamed entrant leaves the game unrated rather than
/// rating the wrong strategy).
/// Register a new player in the roster on disk, so the game they are about to
/// play is rated as theirs. Returns the roster they are now part of and their
/// index in it.
///
/// The two guards are the ones `record_game` already relies on: the league
/// lock, so concurrent workers cannot lose each other's rows, and a refusal to
/// touch a roster a distributed round has already snapshotted. A manifest is
/// an immutable promise about exactly who is playing that round; a person
/// arriving mid-round joins the roster after it instead, and their game goes
/// unrated rather than invalidating jobs already running on other machines.
pub fn register_player(dir: &str) -> Option<(League, usize)> {
    let worker = default_worker_id();
    let _lock = acquire_league_lock(dir, &worker).ok()?;
    let mut league = load_league(dir)?;
    if manifest_path(dir, league.round).exists() {
        return None;
    }
    let index = register_new_player(&mut league);
    save_league_checked(dir, &league).ok()?;
    Some((league, index))
}

pub fn record_game(
    dir: &str,
    placements: &[(String, String)],
    seed: u64,
    turn: u32,
    victory: &str,
) -> Option<League> {
    if placements.len() < 2 {
        return None;
    }
    let worker = default_worker_id();
    let _lock = acquire_league_lock(dir, &worker).ok()?;
    let mut league = load_league(dir)?;
    // A distributed round snapshots this exact roster and rating period.
    // Mixing an ad-hoc exhibition into it would invalidate already-running
    // jobs, so leave that game unrated and let the batch finish intact.
    if manifest_path(dir, league.round).exists() {
        return None;
    }
    let seats: Option<Vec<usize>> = placements
        .iter()
        .map(|(name, _)| league.strategies.iter().position(|s| &s.name == name))
        .collect();
    let outcome = Outcome {
        placements: seats?,
        leaders: placements
            .iter()
            .map(|(_, civilization)| default_leader(civilization))
            .collect(),
        civs: placements.iter().map(|(_, civ)| civ.clone()).collect(),
        // The live-server API supplies a strict placement list. Engine-run
        // league rounds retain score ties in `Outcome::ranks`.
        ranks: (0..placements.len() as u32).collect(),
        won: (0..placements.len()).map(|place| place == 0).collect(),
        seed,
        turn,
        victory: victory.to_string(),
    };
    let names: Vec<String> = placements
        .iter()
        .map(|(name, civ)| format!("{name}@{}@{civ}", default_leader(civ)))
        .collect();
    let round = league.round;
    let calibration_before = league.calibration.clone();
    apply_round(&mut league, &[outcome], false);
    let period_calibration = league.calibration.since(&calibration_before);
    league.round += 1;
    // league.json is authoritative. Derived CSV views may lag after a crash,
    // but a crash can never apply the rating period twice.
    save_league_checked(dir, &league).ok()?;
    append_csv(
        dir,
        "matches.csv",
        "round,seed,turns,victory,placements",
        &[format!(
            "{round},{seed},{turn},{victory},{}",
            names.join("|")
        )],
    );
    let rating_lines: Vec<String> = placements
        .iter()
        .filter_map(|(name, _)| league.strategies.iter().find(|s| &s.name == name))
        .map(|s| {
            format!(
                "{},{},{:.1},{:.1},{:.4},{},{}",
                league.round, s.name, s.rating, s.rd, s.vol, s.games, s.wins
            )
        })
        .collect();
    append_csv(
        dir,
        "ratings.csv",
        "round,name,rating,rd,vol,games,wins",
        &rating_lines,
    );
    append_calibration(dir, league.round, &period_calibration, &league.calibration);
    Some(league)
}

/// Quality-diversity selection: restore or refine the least-represented
/// victory niche using its conservative historical archive plus a strong
/// active parent, then retire the confidently weakest non-elite strategies.
/// Anchors, niche elites, and under-measured strategies are never retired.
fn evolve_league(
    league: &mut League,
    cfg: &LeagueCfg,
    rng: &mut Rng,
) -> (Vec<String>, Vec<String>) {
    let bounds = Weights::bounds();
    let mut parents: Vec<usize> = league
        .active()
        .into_iter()
        .filter(|i| genome_of(&league.strategies[*i].kind).is_some())
        .collect();
    conservative_order(league, &mut parents);
    let pool = (parents.len() / 2).max(1).min(parents.len());
    let mut born = Vec::new();
    if !parents.is_empty() {
        let births = (cfg.max_pop / 4).max(1);
        for birth in 0..births {
            let niche = next_niche(league, cfg, birth);
            let mut archive: Vec<usize> = league
                .strategies
                .iter()
                .enumerate()
                .filter(|(_, strategy)| evolution_niche(&strategy.kind) == Some(niche))
                .map(|(i, _)| i)
                .collect();
            conservative_order(league, &mut archive);
            let archive_pool = (archive.len() / 2).max(1).min(archive.len());
            let pa = if archive.is_empty() {
                parents[rng.below(pool)]
            } else {
                archive[rng.below(archive_pool)]
            };
            let pb = parents[rng.below(pool)];
            let wa = genome_of(&league.strategies[pa].kind).unwrap();
            let wb = genome_of(&league.strategies[pb].kind).unwrap();
            let child = crate::evolve::mutate(
                &crate::evolve::crossover(&wa, &wb, rng),
                rng,
                &bounds,
            );
            // The niche assignment is deliberate rather than inherited by
            // chance: otherwise generalist parents make specialist lanes
            // exponentially unlikely and eventually erase them.
            let target = target_for_niche(niche);
            let name = format!("g{}-{}", league.round, league.strategies.len());
            let kind = StrategyKind::Advanced {
                weights: child,
                target,
            };
            let taken: std::collections::BTreeSet<String> = league
                .strategies
                .iter()
                .map(|s| s.username.clone())
                .collect();
            let pool = username_pool(lane_of(&kind).as_deref());
            let handle = unique_username(pool[rng.below(pool.len())], &taken);
            let mut s = Strategy::new(&name, kind, league.round);
            s.username = handle.clone();
            s.parents = vec![
                league.strategies[pa].name.clone(),
                league.strategies[pb].name.clone(),
            ];
            born.push(handle);
            league.strategies.push(s);
        }
    }
    let mut retired = Vec::new();
    loop {
        let active = league.active();
        if active.len() <= cfg.max_pop {
            break;
        }
        let protected = niche_elites(league);
        let candidate = active
            .into_iter()
            .filter(|i| {
                let s = &league.strategies[*i];
                !s.anchor
                    && !protected.contains(i)
                    && s.games >= MIN_GAMES_TO_RETIRE
                    && s.rd <= MAX_RD_TO_RETIRE
            })
            .min_by(|a, b| {
                upper_confidence(&league.strategies[*a])
                    .total_cmp(&upper_confidence(&league.strategies[*b]))
                    .then_with(|| {
                        league.strategies[*a]
                            .rating
                            .total_cmp(&league.strategies[*b].rating)
                    })
            });
        match candidate {
            Some(i) => {
                league.strategies[i].retired = true;
                retired.push(league.strategies[i].username.clone());
            }
            None => break, // nobody is confidently weak yet; keep the crowd
        }
    }
    (born, retired)
}

/// The rating every player holds before they have finished anything: the
/// Glicko starting point at full uncertainty. Nobody is *without* a rating —
/// a player nothing is known about is a 1500 who has yet to move — so a seat
/// with no league identity at all is shown here rather than left blank.
pub const PROVISIONAL_RATING: f64 = BASE_RATING;
pub const PROVISIONAL_RD: f64 = BASE_RD;

/// What a seat's rating badge should say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayRating {
    pub rating: f64,
    pub rd: f64,
    /// The number is this leader/civilization's own table rather than the
    /// player's overall one.
    pub civ_specific: bool,
    /// No finished game stands behind this number yet: it is the base every
    /// new player starts from, and the first rated result will move it up or
    /// down. The figure is real and comparable, just unearned so far.
    pub provisional: bool,
}

/// The rating to show for an exact player/leader/civilization combination.
/// An unplayed combination uses the player's global prior; after its first
/// game, the combination's own rating is always returned, even provisionally.
///
/// `None` is a seat the league has never heard of — a game running without a
/// roster, or an agent nobody has rated. It still gets the base rating, and
/// the badge says so, because "unrated" and "no rating" are different claims
/// and only the first one is true.
pub fn display_rating(player: Option<&Strategy>, civ: &str) -> DisplayRating {
    display_rating_for(player, &default_leader(civ), civ)
}

pub fn display_rating_for(player: Option<&Strategy>, leader: &str, civ: &str) -> DisplayRating {
    let Some(s) = player else {
        return DisplayRating {
            rating: PROVISIONAL_RATING,
            rd: PROVISIONAL_RD,
            civ_specific: false,
            provisional: true,
        };
    };
    match combination_rating(s, leader, civ) {
        Some(rating) if rating.games > 0 => DisplayRating {
            rating: rating.rating,
            rd: rating.rd,
            civ_specific: true,
            provisional: false,
        },
        _ => DisplayRating {
            rating: s.rating,
            rd: s.rd,
            civ_specific: false,
            provisional: s.games == 0,
        },
    }
}

pub fn display_elo(s: &Strategy, civ: &str) -> (f64, f64, bool) {
    display_elo_for(s, &default_leader(civ), civ)
}

pub fn display_elo_for(s: &Strategy, leader: &str, civ: &str) -> (f64, f64, bool) {
    let shown = display_rating_for(Some(s), leader, civ);
    (shown.rating, shown.rd, shown.civ_specific)
}

/// Seat a table whose civs are already known (civs are fixed per seat in
/// `Game::new`): each seat takes the strongest still-unused active strategy
/// *for its civ*, so different civs field different specialists. Reuses
/// strategies only when the roster is smaller than the table.
pub fn seat_by_civ(league: &League, civs: &[String]) -> Vec<usize> {
    let combinations: Vec<(String, String)> = civs
        .iter()
        .map(|civ| (default_leader(civ), civ.clone()))
        .collect();
    seat_by_leader_civ(league, &combinations)
}

/// Sample each civilization's specialist from its best few rated strategies.
/// Rank weighting (3:2:1 for the default top three) keeps the best entrant
/// most common without making every game the same matchup.
pub fn seat_by_civ_seeded(
    league: &League,
    civs: &[String],
    seed: u64,
    top_n: usize,
) -> Vec<usize> {
    let combinations: Vec<(String, String)> = civs
        .iter()
        .map(|civ| (default_leader(civ), civ.clone()))
        .collect();
    seat_by_leader_civ_seeded(league, &combinations, seed, top_n)
}

pub fn seat_by_leader_civ_seeded(
    league: &League,
    combinations: &[(String, String)],
    seed: u64,
    top_n: usize,
) -> Vec<usize> {
    let active = league.active();
    assert!(!active.is_empty(), "league has no active strategies");
    let mut rng = Rng::new(seed ^ 0x5350_4543_4941_4c49);
    let mut used = BTreeSet::new();
    combinations
        .iter()
        .map(|(leader, civ)| {
            let mut pool: Vec<usize> = if used.len() < active.len() {
                active
                    .iter()
                    .copied()
                    .filter(|candidate| !used.contains(candidate))
                    .collect()
            } else {
                active.clone()
            };
            pool.sort_by(|a, b| {
                let ea = display_elo_for(&league.strategies[*a], leader, civ).0;
                let eb = display_elo_for(&league.strategies[*b], leader, civ).0;
                eb.partial_cmp(&ea).unwrap().then(a.cmp(b))
            });
            pool.truncate(top_n.max(1).min(pool.len()));
            let weights: Vec<f64> = (1..=pool.len()).rev().map(|rank| rank as f64).collect();
            let pick = pool[rng.weighted(&weights)];
            used.insert(pick);
            pick
        })
        .collect()
}

pub fn seat_by_leader_civ(league: &League, combinations: &[(String, String)]) -> Vec<usize> {
    let active = league.active();
    assert!(!active.is_empty(), "league has no active strategies");
    let mut used: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    combinations
        .iter()
        .map(|(leader, civ)| {
            let fresh = active.iter().copied().filter(|i| !used.contains(i));
            let pool: Vec<usize> = if used.len() < active.len() {
                fresh.collect()
            } else {
                active.clone()
            };
            let pick = pool
                .into_iter()
                .max_by(|a, b| {
                    let ea = display_elo_for(&league.strategies[*a], leader, civ).0;
                    let eb = display_elo_for(&league.strategies[*b], leader, civ).0;
                    ea.partial_cmp(&eb).unwrap().then(b.cmp(a))
                })
                .unwrap();
            used.insert(pick);
            pick
        })
        .collect()
}

/// Materialize a strategy as a `Send` AI for the game server's fleet.
pub fn make_send_ai(kind: &StrategyKind, seed: u64) -> Box<dyn Ai + Send> {
    match kind {
        StrategyKind::Builtin { ai } => match ai.as_str() {
            "basic" => Box::new(crate::ai::BasicAi::new()),
            "advanced_v1" => Box::new(AdvancedAi::legacy()),
            "random" => Box::new(crate::ai::RandomAi::new(seed)),
            "advanced_evolved" | "evolved" => Box::new(
                crate::evolve::load_champion("evolved")
                    .map(AdvancedAi::with_weights)
                    .unwrap_or_else(AdvancedAi::new),
            ),
            _ => Box::new(AdvancedAi::new()),
        },
        StrategyKind::Advanced { weights, target } => {
            match target.as_deref().and_then(|t| t.parse::<VictoryTarget>().ok()) {
                Some(t) => Box::new(AdvancedAi::with_weights_and_target(weights.clone(), t)),
                None => Box::new(AdvancedAi::with_weights(weights.clone())),
            }
        }
    }
}

/// One leader/civilization leaderboard. The current ruleset supplies the
/// leader for the compatibility `--civ` interface.
pub fn civ_standings(league: &League, civ: &str) -> String {
    let leader = default_leader(civ);
    let mut rows: Vec<(&Strategy, &CivRating)> = league
        .strategies
        .iter()
        .filter_map(|s| combination_rating(s, &leader, civ).map(|rating| (s, rating)))
        .filter(|(_, c)| c.games > 0)
        .collect();
    if rows.is_empty() {
        return format!("no rated games for {civ} yet\n");
    }
    rows.sort_by(|a, b| b.1.rating.partial_cmp(&a.1.rating).unwrap());
    let mut out = format!("{leader} / {civ} leaderboard (round {}):\n", league.round);
    for (rank, (s, c)) in rows.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {:<18} {:6.0} elo ±{:<4.0} games={:<4} wins={:<3} winrate={:3.0}%  {:<14}{}{}\n",
            rank + 1,
            s.username,
            c.rating,
            c.rd,
            c.games,
            c.wins,
            100.0 * c.wins as f64 / c.games.max(1) as f64,
            s.label(),
            if c.games < CIV_ELO_MIN_GAMES {
                "  provisional"
            } else {
                ""
            },
            if s.retired { "  (retired)" } else { "" },
        ));
    }
    out
}

/// Every observed leader/civilization combination's champion player.
pub fn civ_summary(league: &League) -> String {
    let mut combinations = std::collections::BTreeSet::<(&String, &String)>::new();
    for s in &league.strategies {
        for (leader, civilizations) in &s.leader_elo {
            combinations.extend(civilizations.keys().map(|civ| (leader, civ)));
        }
    }
    if combinations.is_empty() {
        return "no per-leader ratings yet (play some rounds first)\n".to_string();
    }
    let mut out = format!("Best player per leader/civ (round {}):\n", league.round);
    for (leader, civ) in combinations {
        let best = league
            .strategies
            .iter()
            .filter(|s| !s.retired)
            .filter_map(|s| {
                combination_rating(s, leader, civ)
                    .filter(|c| c.games >= CIV_ELO_MIN_GAMES)
                    .map(|c| (s, c))
            })
            .max_by(|a, b| a.1.rating.partial_cmp(&b.1.rating).unwrap());
        match best {
            Some((s, c)) => out.push_str(&format!(
                "  {:<18} {:<12} {:<18} {:6.0} elo ±{:<4.0} ({} games, {:.0}% wins, {})\n",
                leader,
                civ,
                s.username,
                c.rating,
                c.rd,
                c.games,
                100.0 * c.wins as f64 / c.games.max(1) as f64,
                s.label(),
            )),
            None => out.push_str(&format!(
                "  {leader:<18} {civ:<12} (no settled rating yet)\n"
            )),
        }
    }
    out
}

fn append_csv(dir: &str, file: &str, header: &str, lines: &[String]) {
    let path = Path::new(dir).join(file);
    let fresh = !path.exists();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        if fresh {
            let _ = writeln!(f, "{header}");
        }
        for line in lines {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn append_calibration(dir: &str, round: u32, period: &Calibration, cumulative: &Calibration) {
    append_csv(
        dir,
        "calibration.csv",
        "round,comparisons,brier,log_loss,cumulative_comparisons,cumulative_brier,cumulative_log_loss",
        &[format!(
            "{round},{},{:.6},{:.6},{},{:.6},{:.6}",
            period.comparisons,
            period.brier(),
            period.log_loss(),
            cumulative.comparisons,
            cumulative.brier(),
            cumulative.log_loss(),
        )],
    );
}

fn resolve_outcome(league: &League, stored: &StoredOutcome) -> io::Result<Outcome> {
    let placements: Option<Vec<usize>> = stored
        .placements
        .iter()
        .map(|name| {
            league
                .strategies
                .iter()
                .position(|strategy| &strategy.name == name)
        })
        .collect();
    Ok(Outcome {
        placements: placements.ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("result {} references an unknown strategy", stored.job_id),
            )
        })?,
        leaders: stored.leaders.clone(),
        civs: stored.civs.clone(),
        ranks: stored.ranks.clone(),
        won: stored.won.clone(),
        seed: stored.seed,
        turn: stored.turn,
        victory: stored.victory.clone(),
    })
}

struct FinalizedRound {
    league: League,
    round: u32,
    games: usize,
    born: Vec<String>,
    retired: Vec<String>,
}

/// Apply a complete immutable result set once. The short state lock covers
/// validation, one simultaneous Glicko update, selection, and checkpointing;
/// expensive simulation never runs while this lock is held.
fn try_finalize_round(
    cfg: &LeagueCfg,
    expected_manifest: &RoundManifest,
) -> io::Result<Option<FinalizedRound>> {
    let _lock = acquire_league_lock(&cfg.dir, &cfg.worker_id)?;
    let mut league = load_league(&cfg.dir).ok_or_else(|| {
        io::Error::new(
            ErrorKind::NotFound,
            format!("missing {}/league.json", cfg.dir),
        )
    })?;
    if league.round > expected_manifest.round {
        return Ok(None); // another worker already committed it
    }
    if league.round != expected_manifest.round {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "league round moved behind its work manifest",
        ));
    }
    let manifest: RoundManifest = read_json(&manifest_path(&cfg.dir, league.round))?;
    validate_manifest(&manifest, &league)?;
    if &manifest != expected_manifest {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "round manifest changed after work was claimed",
        ));
    }

    let mut stored = Vec::with_capacity(manifest.jobs.len());
    for job in &manifest.jobs {
        let result = match read_json::<StoredOutcome>(&result_path(&cfg.dir, league.round, &job.id))
        {
            Ok(result) => result,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        validate_result(&manifest, job, &result)?;
        stored.push(result);
    }
    let outcomes: Vec<Outcome> = stored
        .iter()
        .map(|result| resolve_outcome(&league, result))
        .collect::<io::Result<_>>()?;
    let round = league.round;
    let calibration_before = league.calibration.clone();
    apply_round(&mut league, &outcomes, true);
    let period_calibration = league.calibration.since(&calibration_before);
    league.round += 1;

    let mut effective_cfg = cfg.clone();
    effective_cfg.seed = manifest.config.league_seed;
    effective_cfg.evolve_every = manifest.config.evolve_every;
    effective_cfg.max_pop = manifest.config.max_pop;
    let mut rng = round_rng(effective_cfg.seed, round);
    let (born, retired) =
        if effective_cfg.evolve_every > 0 && league.round % effective_cfg.evolve_every == 0 {
            evolve_league(&mut league, &effective_cfg, &mut rng)
        } else {
            (Vec::new(), Vec::new())
        };

    // Commit the source of truth before derived human-readable logs. If the
    // process dies below, ratings still cannot be applied twice.
    save_league_checked(&cfg.dir, &league)?;
    let match_lines: Vec<String> = stored
        .iter()
        .map(|outcome| {
            let names: Vec<String> = outcome
                .placements
                .iter()
                .zip(&outcome.civs)
                .map(|(name, civ)| format!("{name}@{civ}"))
                .collect();
            format!(
                "{round},{},{},{},{}",
                outcome.seed,
                outcome.turn,
                outcome.victory,
                names.join("|")
            )
        })
        .collect();
    let rating_lines: Vec<String> = league
        .active()
        .into_iter()
        .map(|index| {
            let strategy = &league.strategies[index];
            format!(
                "{},{},{:.1},{:.1},{:.4},{},{}",
                league.round,
                strategy.name,
                strategy.rating,
                strategy.rd,
                strategy.vol,
                strategy.games,
                strategy.wins
            )
        })
        .collect();
    append_csv(
        &cfg.dir,
        "matches.csv",
        "round,seed,turns,victory,placements",
        &match_lines,
    );
    append_csv(
        &cfg.dir,
        "ratings.csv",
        "round,name,rating,rd,vol,games,wins",
        &rating_lines,
    );
    append_calibration(
        &cfg.dir,
        league.round,
        &period_calibration,
        &league.calibration,
    );
    let _ = atomic_write_json(
        &round_dir(&cfg.dir, round).join("finalized.json"),
        &serde_json::json!({
            "round": round,
            "next_round": league.round,
            "games": stored.len(),
            "born": born,
            "retired": retired,
            "calibration": period_calibration,
        }),
    );
    Ok(Some(FinalizedRound {
        league,
        round,
        games: stored.len(),
        born,
        retired,
    }))
}

fn execute_claims(
    cfg: &LeagueCfg,
    manifest: &RoundManifest,
    claimed: &[ClaimedJob],
) -> io::Result<()> {
    let results = crate::parallel::map(claimed.len(), cfg.jobs.max(1), |index| {
        play_job(manifest, &claimed[index].job, &cfg.worker_id)
    });
    for (claim, result) in claimed.iter().zip(results) {
        validate_result(manifest, &claim.job, &result)?;
        publish_json_once(
            &result_path(&cfg.dir, manifest.round, &claim.job.id),
            &result,
        )?;
        release_claim(cfg, manifest.round, claim);
    }
    Ok(())
}

pub fn standings(league: &League) -> String {
    let mut order: Vec<&Strategy> = league.strategies.iter().collect();
    order.sort_by(|a, b| {
        a.retired
            .cmp(&b.retired)
            .then(b.rating.partial_cmp(&a.rating).unwrap())
    });
    let mut out = format!("League players after round {}:\n", league.round);
    if league.calibration.comparisons > 0 {
        out.push_str(&format!(
            "Prediction calibration: {} pairwise results, Brier {:.4}, log loss {:.4}\n",
            league.calibration.comparisons,
            league.calibration.brier(),
            league.calibration.log_loss(),
        ));
    }
    for (rank, s) in order.iter().enumerate() {
        let status = if s.retired {
            "retired"
        } else if s.human {
            "person"
        } else if s.anchor {
            "anchor"
        } else {
            "active"
        };
        out.push_str(&format!(
            "  {:>2}. {:<18} {:6.0} elo ±{:<4.0} {:<14} games={:<5} wins={:<4} winrate={:3.0}%  born r{:<3} {:<7} [{}]\n",
            rank + 1,
            s.username,
            s.rating,
            s.rd,
            s.label(),
            s.games,
            s.wins,
            100.0 * s.wins as f64 / s.games.max(1) as f64,
            s.born_round,
            status,
            s.name,
        ));
    }
    out
}

pub fn try_run_league(cfg: &LeagueCfg) -> io::Result<League> {
    if cfg.players_per_game < 2
        || cfg.games_per_round == 0
        || cfg.max_pop == 0
        || cfg.worker_id.trim().is_empty()
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "league needs at least two players, one game, one strategy slot, and a worker ID",
        ));
    }
    let initial = {
        let _lock = acquire_league_lock(&cfg.dir, &cfg.worker_id)?;
        match load_league(&cfg.dir) {
            Some(league) => league,
            None => {
                let league = seed_league(&cfg.dir);
                save_league_checked(&cfg.dir, &league)?;
                league
            }
        }
    };
    let target_round = initial.round.saturating_add(cfg.rounds);
    let mut presence = if initial.round < target_round {
        let presence = register_worker(cfg)?;
        // Give concurrently launched machines a short window to register so
        // the first process does not reserve the entire rating period.
        thread::sleep(Duration::from_millis(WORKER_DISCOVERY_MILLIS));
        Some(presence)
    } else {
        None
    };
    let mut latest = initial;
    while latest.round < target_round {
        let manifest = {
            let _lock = acquire_league_lock(&cfg.dir, &cfg.worker_id)?;
            latest = load_league(&cfg.dir).ok_or_else(|| {
                io::Error::new(ErrorKind::NotFound, "league disappeared while working")
            })?;
            if latest.round >= target_round {
                break;
            }
            load_or_create_manifest(&latest, cfg)?
        };

        if let Some(presence) = &mut presence {
            presence.refresh()?;
        }
        let claimed = claim_jobs(cfg, &manifest)?;
        if !claimed.is_empty() {
            if cfg.verbose {
                println!(
                    "worker {}: round {} claimed {} of {} mirrored games",
                    cfg.worker_id,
                    manifest.round,
                    claimed.len(),
                    manifest.jobs.len()
                );
            }
            execute_claims(cfg, &manifest, &claimed)?;
        }

        let all_results_present = manifest
            .jobs
            .iter()
            .all(|job| result_path(&cfg.dir, manifest.round, &job.id).exists());
        let finalized = if all_results_present {
            try_finalize_round(cfg, &manifest)?
        } else {
            None
        };
        match finalized {
            Some(done) => {
                latest = done.league;
                if cfg.verbose {
                    let leader = latest
                        .active()
                        .into_iter()
                        .max_by(|a, b| {
                            latest.strategies[*a]
                                .rating
                                .total_cmp(&latest.strategies[*b].rating)
                        })
                        .unwrap();
                    println!(
                        "round {:>3}: {} mirrored games; leader {} {:.1} ±{:.1}{}{}",
                        done.round,
                        done.games,
                        latest.strategies[leader].username,
                        latest.strategies[leader].rating,
                        latest.strategies[leader].rd,
                        if done.born.is_empty() {
                            String::new()
                        } else {
                            format!("; born {:?}", done.born)
                        },
                        if done.retired.is_empty() {
                            String::new()
                        } else {
                            format!("; retired {:?}", done.retired)
                        },
                    );
                }
            }
            None => {
                latest = load_league(&cfg.dir).unwrap_or(latest);
                if claimed.is_empty() && latest.round < target_round {
                    // Other workers own the remaining leases. Poll cheaply;
                    // a crashed owner is reclaimed automatically at expiry.
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
    latest = load_league(&cfg.dir).unwrap_or(latest);
    if cfg.verbose {
        println!();
        print!("{}", standings(&latest));
    }
    Ok(latest)
}

pub fn run_league(cfg: &LeagueCfg) -> League {
    try_run_league(cfg).unwrap_or_else(|error| panic!("league failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nobody is without a rating.
    ///
    /// "Unrated" is a claim about a player's *record*, not about whether they
    /// have a number: everyone enters at 1500 with the deviation to say how
    /// little that means yet, and the first result moves it. A seat the
    /// league has never heard of is that same 1500, so a badge can always
    /// show a figure instead of a dash.
    #[test]
    fn an_unplayed_seat_is_a_provisional_1500_rather_than_no_rating() {
        let unknown = display_rating(None, "Rome");
        assert_eq!((unknown.rating, unknown.rd), (1500.0, 350.0));
        assert!(unknown.provisional && !unknown.civ_specific);

        // A registered player who has finished nothing: their own row, but
        // still the number they started with.
        let mut fresh = Strategy::new(
            "newcomer",
            StrategyKind::Builtin { ai: "advanced".to_string() },
            0,
        );
        let shown = display_rating(Some(&fresh), "Rome");
        assert_eq!((shown.rating, shown.rd), (1500.0, 350.0));
        assert!(shown.provisional);

        // One finished game and the global number is earned, even though this
        // civilization's own table has not been opened yet.
        fresh.games = 1;
        fresh.rating = 1532.0;
        let shown = display_rating(Some(&fresh), "Rome");
        assert_eq!(shown.rating, 1532.0);
        assert!(!shown.provisional && !shown.civ_specific);

        // And once the combination has been played, it speaks for itself.
        fresh
            .leader_elo
            .entry(default_leader("Rome"))
            .or_default()
            .insert(
                "Rome".to_string(),
                CivRating { rating: 1610.0, rd: 180.0, games: 3, ..CivRating::default() },
            );
        let shown = display_rating(Some(&fresh), "Rome");
        assert_eq!((shown.rating, shown.rd), (1610.0, 180.0));
        assert!(shown.civ_specific && !shown.provisional);
        // A different civilization still falls back to the global rating.
        assert!(!display_rating(Some(&fresh), "Egypt").civ_specific);
    }

    /// The worked example from Glickman's Glicko-2 paper: 1500/200/0.06
    /// beating 1400/30 then losing to 1550/100 and 1700/300 in one period
    /// must land on 1464.06 / 151.52 / 0.05999.
    #[test]
    fn glicko2_matches_glickman_paper_example() {
        let player = Glicko {
            mu: 0.0,
            phi: 200.0 / SCALE,
            sigma: 0.06,
        };
        let opponent = |r: f64, rd: f64| Glicko {
            mu: (r - 1500.0) / SCALE,
            phi: rd / SCALE,
            sigma: 0.06,
        };
        let results = vec![
            (opponent(1400.0, 30.0), 1.0, 1.0),
            (opponent(1550.0, 100.0), 0.0, 1.0),
            (opponent(1700.0, 300.0), 0.0, 1.0),
        ];
        let out = rate(player, &results);
        let rating = 1500.0 + SCALE * out.mu;
        let rd = SCALE * out.phi;
        assert!((rating - 1464.06).abs() < 0.1, "rating {rating}");
        assert!((rd - 151.52).abs() < 0.1, "rd {rd}");
        assert!((out.sigma - 0.05999).abs() < 0.0002, "vol {}", out.sigma);
    }

    /// One four-player finish provides three correlated pairwise comparisons,
    /// not three independent games. Weighting them to one effective result
    /// prevents multiplayer tables from manufacturing false precision.
    #[test]
    fn multiplayer_pairwise_results_have_one_games_worth_of_information() {
        let player = Glicko {
            mu: 0.0,
            phi: BASE_RD / SCALE,
            sigma: BASE_VOL,
        };
        let opponent = Glicko {
            mu: 0.0,
            phi: BASE_RD / SCALE,
            sigma: BASE_VOL,
        };
        let single = rate(player, &[(opponent, 1.0, 1.0)]);
        let multiplayer = rate(
            player,
            &[
                (opponent, 1.0, 1.0 / 3.0),
                (opponent, 1.0, 1.0 / 3.0),
                (opponent, 1.0, 1.0 / 3.0),
            ],
        );
        let falsely_independent = rate(
            player,
            &[
                (opponent, 1.0, 1.0),
                (opponent, 1.0, 1.0),
                (opponent, 1.0, 1.0),
            ],
        );
        assert!((single.mu - multiplayer.mu).abs() < 1e-12);
        assert!((single.phi - multiplayer.phi).abs() < 1e-12);
        assert!(falsely_independent.phi < multiplayer.phi);
    }

    #[test]
    fn idle_period_grows_uncertainty_but_not_rating() {
        let player = Glicko {
            mu: 0.5,
            phi: 80.0 / SCALE,
            sigma: 0.06,
        };
        let out = rate(player, &[]);
        assert_eq!(out.mu, 0.5);
        assert!(out.phi > player.phi);
        assert!(out.phi <= BASE_RD / SCALE);
    }

    #[test]
    fn matchup_predictions_are_symmetric_and_include_both_deviations() {
        let a = Glicko {
            mu: 1.0,
            phi: 40.0 / SCALE,
            sigma: BASE_VOL,
        };
        let b = Glicko {
            mu: -0.5,
            phi: 250.0 / SCALE,
            sigma: BASE_VOL,
        };
        let ab = matchup_expectation(a, b);
        let ba = matchup_expectation(b, a);
        assert!((ab + ba - 1.0).abs() < 1e-12);
        assert!(ab > 0.5 && ab < 1.0);
    }

    #[test]
    fn a_new_leader_table_uses_global_skill_as_an_uncertain_prior() {
        let global = Glicko {
            mu: (1800.0 - BASE_RATING) / SCALE,
            phi: 50.0 / SCALE,
            sigma: BASE_VOL,
        };
        let prior = leader_prior(global);
        assert!((prior.rating - 1800.0).abs() < 1e-9);
        assert!(prior.rd > 200.0 && prior.rd < BASE_RD);
        assert_eq!((prior.games, prior.wins), (0, 0));
    }

    #[test]
    fn equal_scores_are_glicko_draws_and_not_seat_order_wins() {
        let builtin = |ai: &str| StrategyKind::Builtin { ai: ai.into() };
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("a", builtin("advanced"), 0),
                Strategy::new("b", builtin("basic"), 0),
            ],
            calibration: Calibration::default(),
        };
        let outcome = Outcome {
            placements: vec![0, 1],
            leaders: vec!["Trajan".into(), "Cleopatra".into()],
            civs: vec!["Rome".into(), "Egypt".into()],
            ranks: vec![0, 0],
            won: vec![false, false],
            seed: 0,
            turn: 50,
            victory: String::new(),
        };
        apply_round(&mut league, &[outcome], true);
        assert!((league.strategies[0].rating - BASE_RATING).abs() < 1e-9);
        assert!((league.strategies[1].rating - BASE_RATING).abs() < 1e-9);
        assert_eq!(league.strategies[0].wins + league.strategies[1].wins, 0);
        assert_eq!(league.calibration.comparisons, 1);
        assert!(league.calibration.brier() < 1e-12);
        assert!((league.calibration.log_loss() - std::f64::consts::LN_2).abs() < 1e-12);
    }

    #[test]
    fn winners_gain_and_losers_lose() {
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("a", StrategyKind::Builtin { ai: "basic".into() }, 0),
                Strategy::new("b", StrategyKind::Builtin { ai: "basic".into() }, 0),
            ],
            calibration: Calibration::default(),
        };
        let outcomes = vec![Outcome {
            placements: vec![0, 1],
            leaders: vec!["Trajan".into(), "Cleopatra".into()],
            civs: vec!["Rome".into(), "Egypt".into()],
            ranks: vec![0, 1],
            won: vec![true, false],
            seed: 0,
            turn: 10,
            victory: "score".into(),
        }];
        apply_round(&mut league, &outcomes, true);
        assert!(league.strategies[0].rating > BASE_RATING);
        assert!(league.strategies[1].rating < BASE_RATING);
        assert_eq!(league.strategies[0].wins, 1);
        assert_eq!(league.strategies[0].games, 1);
        // the same result also lands on each exact leader/civ table
        let rome = &league.strategies[0].leader_elo["Trajan"]["Rome"];
        let egypt = &league.strategies[1].leader_elo["Cleopatra"]["Egypt"];
        assert!(rome.rating > BASE_RATING && rome.games == 1 && rome.wins == 1);
        assert!(egypt.rating < BASE_RATING && egypt.games == 1 && egypt.wins == 0);
        assert!(league.strategies[0].leader_elo.get("Cleopatra").is_none());
    }

    /// A finished game rated on its own moves only the strategies that
    /// played it. Ageing the rest would be right for a league round, which
    /// schedules everyone, but a six-seat game is not an idle period for the
    /// twenty strategies that could never have entered it.
    #[test]
    fn a_single_recorded_game_leaves_absent_strategies_alone() {
        let builtin = |ai: &str| StrategyKind::Builtin { ai: ai.into() };
        let mut league = League {
            round: 7,
            strategies: vec![
                Strategy::new("a", builtin("advanced"), 0),
                Strategy::new("b", builtin("basic"), 0),
                Strategy::new("bench", builtin("random"), 0),
            ],
            calibration: Calibration::default(),
        };
        let bench_before = (league.strategies[2].rating, league.strategies[2].rd);
        let outcomes = vec![Outcome {
            placements: vec![0, 1],
            leaders: vec!["Trajan".into(), "Cleopatra".into()],
            civs: vec!["Rome".into(), "Egypt".into()],
            ranks: vec![0, 1],
            won: vec![true, false],
            seed: 3,
            turn: 90,
            victory: "science".into(),
        }];
        apply_round(&mut league, &outcomes, false);
        assert!(league.strategies[0].rating > BASE_RATING);
        assert!(league.strategies[1].rating < BASE_RATING);
        let bench = &league.strategies[2];
        assert_eq!((bench.rating, bench.rd), bench_before);
        assert_eq!(bench.games, 0);
    }

    /// Somebody who sits down to play joins the table as themselves. They are
    /// rated by the same arithmetic as the agents and they appear in the
    /// standings, but nothing may ever schedule, breed, retire or seat them:
    /// a league round that dealt a person's row to a worker would play games
    /// in their name that they never touched.
    #[test]
    fn a_registered_person_is_a_player_but_never_an_entrant() {
        let builtin = |ai: &str| StrategyKind::Builtin { ai: ai.into() };
        let mut league = League {
            round: 4,
            strategies: vec![
                Strategy::new("advanced", builtin("advanced"), 0),
                Strategy::new("basic", builtin("basic"), 0),
            ],
            calibration: Calibration::default(),
        };
        ensure_usernames(&mut league);

        let first = register_new_player(&mut league);
        let second = register_new_player(&mut league);
        assert_ne!(first, second);
        assert_eq!(league.strategies[first].name, "player");
        assert_eq!(league.strategies[first].username, "Player");
        assert_eq!(league.strategies[second].username, "Player2");
        assert_eq!(league.strategies[first].rating, BASE_RATING);
        assert_eq!(league.strategies[first].games, 0);
        assert_eq!(league.strategies[first].born_round, 4);
        assert_eq!(league.strategies[first].label(), "human");

        assert_eq!(league.active(), vec![0, 1]);
        assert_eq!(league.humans(), vec![first, second]);
        assert!(seat_by_civ(&league, &["Rome".into(), "Egypt".into()])
            .iter()
            .all(|seated| !league.strategies[*seated].human));
        assert!(standings(&league).contains("person"));
    }

    /// A person is registered before their game starts, so the result has a
    /// name of their own to be filed under. The roster on disk is the one
    /// that changes, and a second person is a second player.
    #[test]
    fn registering_a_player_persists_a_new_row() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-league-register-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let dir = dir.to_str().unwrap();
        let _ = fs::remove_dir_all(dir);
        let builtin = |ai: &str| StrategyKind::Builtin { ai: ai.into() };
        save_league(
            dir,
            &League {
                round: 3,
                strategies: vec![Strategy::new("advanced", builtin("advanced"), 0)],
                calibration: Calibration::default(),
            },
        );

        let (league, index) = register_player(dir).expect("registered");
        assert_eq!(index, 1);
        assert!(league.strategies[index].human);
        let reloaded = load_league(dir).expect("roster on disk");
        assert_eq!(reloaded.strategies.len(), 2);
        assert_eq!(reloaded.strategies[index].username, "Player");
        // The round is untouched: registering is not a rating period.
        assert_eq!(reloaded.round, 3);

        let (_, next) = register_player(dir).expect("registered again");
        assert_eq!(load_league(dir).unwrap().strategies[next].username, "Player2");

        // A game a person actually finishes rates them, and rates them alone
        // among the two of them — nothing was credited to `advanced`.
        let placements = vec![
            ("player".to_string(), "Rome".to_string()),
            ("advanced".to_string(), "Egypt".to_string()),
        ];
        let rated = record_game(dir, &placements, 9, 140, "science").expect("rated");
        let person = rated.strategies.iter().find(|s| s.name == "player").unwrap();
        assert_eq!((person.games, person.wins), (1, 1));
        assert!(person.rating > BASE_RATING);
        assert_eq!(rated.strategies.iter().find(|s| s.name == "player2").unwrap().games, 0);
        fs::remove_dir_all(dir).unwrap();
    }

    /// `record_game` is the live server's whole path to a moving table: it
    /// must persist, keep counting across games, and rate by name so a
    /// roster that changed under a long game is not overwritten with a
    /// stale one.
    #[test]
    fn recording_a_game_persists_and_accumulates() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-league-record-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let dir = dir.to_str().unwrap();
        let _ = fs::remove_dir_all(dir);
        let builtin = |ai: &str| StrategyKind::Builtin { ai: ai.into() };
        let mut seeded = League {
            round: 12,
            strategies: vec![
                Strategy::new("a", builtin("advanced"), 0),
                Strategy::new("b", builtin("basic"), 0),
            ],
            calibration: Calibration::default(),
        };
        seeded.strategies[1].rating = 1600.0;
        save_league(dir, &seeded);

        let placements = vec![
            ("a".to_string(), "Rome".to_string()),
            ("b".to_string(), "Egypt".to_string()),
        ];
        let first = record_game(dir, &placements, 5, 120, "culture").expect("rated");
        assert_eq!(first.round, 13);
        assert!(first.strategies[0].rating > BASE_RATING);
        assert!(first.strategies[1].rating < 1600.0);
        assert_eq!(first.strategies[0].leader_elo["Trajan"]["Rome"].wins, 1);

        // Reloaded from disk, not from the caller's copy.
        let second = record_game(dir, &placements, 6, 130, "culture").expect("rated");
        assert_eq!(second.round, 14);
        assert_eq!(second.strategies[0].games, 2);
        assert!(second.strategies[0].rating > first.strategies[0].rating);
        assert_eq!(second.calibration.comparisons, 2);
        assert_eq!(
            load_league(dir).unwrap().strategies[0].rating,
            second.strategies[0].rating
        );
        let matches = fs::read_to_string(Path::new(dir).join("matches.csv")).unwrap();
        assert_eq!(matches.lines().count(), 3, "header plus one row per game");
        assert!(matches.contains("a@Trajan@Rome|b@Cleopatra@Egypt"));
        let calibration = fs::read_to_string(Path::new(dir).join("calibration.csv")).unwrap();
        assert_eq!(calibration.lines().count(), 3);
        assert!(calibration.starts_with("round,comparisons,brier,log_loss,"));

        // A name the roster no longer carries leaves the table untouched.
        let unknown = vec![
            ("a".to_string(), "Rome".to_string()),
            ("ghost".to_string(), "Egypt".to_string()),
        ];
        assert!(record_game(dir, &unknown, 7, 140, "score").is_none());
        assert_eq!(load_league(dir).unwrap().round, 14);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn older_league_snapshots_begin_with_an_empty_calibration_audit() {
        let old = r#"{"round":7,"strategies":[]}"#;
        let league: League = serde_json::from_str(old).unwrap();
        assert_eq!(league.round, 7);
        assert_eq!(league.calibration.comparisons, 0);
    }

    #[test]
    fn civilization_only_snapshots_migrate_to_the_ruleset_leader() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-league-migrate-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("league.json"),
            r#"{"round":1,"strategies":[{"name":"a","kind":{"Builtin":{"ai":"basic"}},"rating":1500.0,"rd":350.0,"vol":0.06,"games":1,"wins":1,"civ_elo":{"Rome":{"rating":1600.0,"rd":200.0,"vol":0.06,"games":1,"wins":1}},"born_round":0}]}"#,
        )
        .unwrap();
        let league = load_league(dir.to_str().unwrap()).unwrap();
        assert_eq!(
            league.strategies[0].leader_elo["Trajan"]["Rome"].rating,
            1600.0
        );
        assert!(league.strategies[0].legacy_civ_elo.is_empty());
        save_league(dir.to_str().unwrap(), &league);
        let saved = fs::read_to_string(dir.join("league.json")).unwrap();
        assert!(saved.contains("\"leader_elo\""));
        assert!(!saved.contains("\"civ_elo\""));
        fs::remove_dir_all(dir).unwrap();
    }

    /// Seating by leader/civ prefers each combination's specialist and never
    /// doubles a strategy up while unused ones remain.
    #[test]
    fn seat_by_civ_prefers_leader_specialists() {
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("gen", StrategyKind::Builtin { ai: "advanced".into() }, 0),
                Strategy::new(
                    "rome-expert",
                    StrategyKind::Advanced {
                        weights: Weights::default(),
                        target: Some("domination".into()),
                    },
                    0,
                ),
            ],
            calibration: Calibration::default(),
        };
        league.strategies[0].rating = 1650.0; // globally stronger
        league.strategies[1].rating = 1450.0;
        league.strategies[1].leader_elo.insert(
            "Trajan".into(),
            BTreeMap::from([(
                "Rome".into(),
                CivRating {
                    rating: 1750.0,
                    games: CIV_ELO_MIN_GAMES,
                    ..CivRating::default()
                },
            )]),
        );
        let seats = seat_by_civ(&league, &["Rome".into(), "Egypt".into()]);
        assert_eq!(seats, vec![1, 0], "Rome goes to its specialist");
        let (elo, _, civ_specific) = display_elo(&league.strategies[1], "Rome");
        assert!(civ_specific && (elo - 1750.0).abs() < 1e-9);
        // below the evidence bar the global rating stands in
        let (elo, _, civ_specific) = display_elo(&league.strategies[1], "Egypt");
        assert!(!civ_specific && (elo - 1450.0).abs() < 1e-9);
    }

    #[test]
    fn seeded_seating_varies_within_each_civilizations_top_few() {
        let mut league = League {
            round: 0,
            strategies: (0..4)
                .map(|index| {
                    Strategy::new(
                        &format!("candidate-{index}"),
                        StrategyKind::Builtin {
                            ai: "advanced".into(),
                        },
                        0,
                    )
                })
                .collect(),
            calibration: Calibration::default(),
        };
        for (index, strategy) in league.strategies.iter_mut().enumerate() {
            strategy.rating = 1_800.0 - index as f64 * 100.0;
        }
        let mut seen = BTreeSet::new();
        for seed in 0..128 {
            seen.insert(seat_by_civ_seeded(&league, &["Byzantium".into()], seed, 3)[0]);
        }
        assert_eq!(seen, BTreeSet::from([0, 1, 2]));
        assert!(!seen.contains(&3), "the fourth-rated strategy is outside the pool");
        assert_eq!(
            seat_by_civ_seeded(&league, &["Byzantium".into()], 17, 3),
            seat_by_civ_seeded(&league, &["Byzantium".into()], 17, 3),
            "a game seed must reproduce its specialist"
        );
    }

    #[test]
    fn schedule_covers_roster_and_fills_tables() {
        let cfg = LeagueCfg {
            games_per_round: 6,
            players_per_game: 4,
            ..LeagueCfg::default()
        };
        let active: Vec<usize> = (0..9).collect();
        let mut rng = Rng::new(7);
        let tables = schedule(&active, &cfg, &mut rng);
        assert_eq!(tables.len(), 6);
        let mut seen = std::collections::BTreeSet::new();
        for t in &tables {
            assert_eq!(t.len(), 4);
            assert!(t.iter().any(|s| *s != t[0]), "table of clones");
            seen.extend(t.iter().copied());
        }
        // two dealt passes over 9 strategies fill 24 seats: everyone plays
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn manifests_rotate_each_matchup_through_every_starting_seat() {
        let strategies = (0..4)
            .map(|index| {
                Strategy::new(
                    &format!("strategy-{index}"),
                    StrategyKind::Builtin {
                        ai: "advanced".into(),
                    },
                    0,
                )
            })
            .collect();
        let league = League {
            round: 7,
            strategies,
            calibration: Calibration::default(),
        };
        let cfg = LeagueCfg {
            games_per_round: 4,
            players_per_game: 4,
            ..LeagueCfg::default()
        };
        let manifest = build_manifest(&league, &cfg);
        assert_eq!(manifest.jobs.len(), 4);
        assert!(manifest
            .jobs
            .iter()
            .all(|job| job.seed == manifest.jobs[0].seed));
        for strategy in &league.strategies {
            for seat in 0..4 {
                assert_eq!(
                    manifest
                        .jobs
                        .iter()
                        .filter(|job| job.table[seat] == strategy.name)
                        .count(),
                    1,
                    "{} did not play seat {seat} exactly once",
                    strategy.name,
                );
            }
        }

        let rounded = build_manifest(
            &league,
            &LeagueCfg {
                games_per_round: 5,
                players_per_game: 4,
                ..cfg
            },
        );
        assert_eq!(rounded.jobs.len(), 8, "partial mirror series must round up");
    }

    #[test]
    fn result_validation_rejects_wrong_workers_rosters_and_ranks() {
        let league = League {
            round: 0,
            strategies: (0..2)
                .map(|index| {
                    Strategy::new(
                        &format!("strategy-{index}"),
                        StrategyKind::Builtin { ai: "advanced".into() },
                        0,
                    )
                })
                .collect(),
            calibration: Calibration::default(),
        };
        let cfg = LeagueCfg {
            games_per_round: 2,
            players_per_game: 2,
            ..LeagueCfg::default()
        };
        let manifest = build_manifest(&league, &cfg);
        let job = &manifest.jobs[0];
        let mut result = StoredOutcome {
            schema_version: WORK_SCHEMA_VERSION,
            engine: manifest.engine.clone(),
            worker: "machine-a".into(),
            round: manifest.round,
            job_id: job.id.clone(),
            placements: job.table.clone(),
            leaders: vec!["Trajan".into(), "Cleopatra".into()],
            civs: vec!["Rome".into(), "Egypt".into()],
            ranks: vec![0, 1],
            won: vec![true, false],
            seed: job.seed,
            turn: 80,
            victory: "science".into(),
        };
        assert!(validate_result(&manifest, job, &result).is_ok());
        result.worker.clear();
        assert!(validate_result(&manifest, job, &result).is_err());
        result.worker = "machine-a".into();
        result.placements[0] = "ghost".into();
        assert!(validate_result(&manifest, job, &result).is_err());
        result.placements = job.table.clone();
        result.ranks = vec![0, 0];
        assert!(validate_result(&manifest, job, &result).is_err());
    }

    #[test]
    fn selection_breeds_from_leaders_and_retires_confident_losers() {
        let mut league = League {
            round: 8,
            strategies: Vec::new(),
            calibration: Calibration::default(),
        };
        for i in 0..6 {
            let mut s = Strategy::new(
                &format!("s{i}"),
                StrategyKind::Advanced {
                    weights: Weights::default(),
                    target: None,
                },
                0,
            );
            s.rating = 1600.0 - 40.0 * i as f64;
            s.rd = 60.0;
            s.games = 30;
            league.strategies.push(s);
        }
        league.strategies[0].anchor = true;
        // an under-measured newcomer that must survive despite a bad rating
        let mut newborn = Strategy::new(
            "newborn",
            StrategyKind::Advanced {
                weights: Weights::default(),
                target: None,
            },
            7,
        );
        newborn.rating = 1200.0;
        newborn.rd = 300.0;
        newborn.games = 3;
        league.strategies.push(newborn);

        let cfg = LeagueCfg {
            max_pop: 7,
            ..LeagueCfg::default()
        };
        ensure_usernames(&mut league);
        let handle = |league: &League, name: &str| {
            league
                .strategies
                .iter()
                .find(|s| s.name == name)
                .unwrap()
                .username
                .clone()
        };
        let newborn_handle = handle(&league, "newborn");
        let anchor_handle = handle(&league, "s0");
        let mut rng = Rng::new(3);
        let (born, retired) = evolve_league(&mut league, &cfg, &mut rng);
        assert!(!born.is_empty());
        assert!(!retired.contains(&newborn_handle));
        assert!(!retired.contains(&anchor_handle), "anchor retired");
        // offspring exist, are active, carry lineage, and have a handle
        let child = league
            .strategies
            .iter()
            .find(|s| born.contains(&s.username))
            .unwrap();
        assert!(!child.username.is_empty());
        assert_eq!(child.parents.len(), 2);
        assert!(!child.retired);
        assert_eq!(child.rd, BASE_RD);
        // roster trimmed back to cap (retirees had games and low rd)
        assert!(league.active().len() <= cfg.max_pop.max(7));
    }

    #[test]
    fn breeding_uses_conservative_skill_instead_of_noisy_point_rating() {
        let mut league = League {
            round: 4,
            strategies: Vec::new(),
            calibration: Calibration::default(),
        };
        for (name, rating, rd) in [
            ("noisy-leader", 1900.0, 200.0),
            ("proven-first", 1800.0, 30.0),
            ("proven-second", 1700.0, 30.0),
            ("settled-fourth", 1600.0, 30.0),
        ] {
            let mut s = Strategy::new(
                name,
                StrategyKind::Advanced {
                    weights: Weights::default(),
                    target: None,
                },
                0,
            );
            s.rating = rating;
            s.rd = rd;
            league.strategies.push(s);
        }
        ensure_usernames(&mut league);
        let cfg = LeagueCfg {
            max_pop: 4,
            ..LeagueCfg::default()
        };
        let mut rng = Rng::new(9);
        let (born, _) = evolve_league(&mut league, &cfg, &mut rng);
        let child = league
            .strategies
            .iter()
            .find(|s| born.contains(&s.username))
            .unwrap();
        assert!(child
            .parents
            .iter()
            .all(|p| p == "proven-first" || p == "proven-second"));
    }

    #[test]
    fn selection_restores_missing_niches_from_the_historical_archive() {
        let advanced = |target: Option<&str>| StrategyKind::Advanced {
            weights: Weights::default(),
            target: target.map(str::to_string),
        };
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("generalist-a", advanced(None), 0),
                Strategy::new("generalist-b", advanced(None), 0),
                Strategy::new("retired-science", advanced(Some("science")), 0),
            ],
            calibration: Calibration::default(),
        };
        league.strategies[0].rating = 1750.0;
        league.strategies[0].rd = 30.0;
        league.strategies[1].rating = 1650.0;
        league.strategies[1].rd = 30.0;
        league.strategies[2].rating = 1600.0;
        league.strategies[2].rd = 40.0;
        league.strategies[2].retired = true;
        ensure_usernames(&mut league);

        let cfg = LeagueCfg {
            max_pop: 12,
            ..LeagueCfg::default()
        };
        let mut rng = Rng::new(31);
        let (born, _) = evolve_league(&mut league, &cfg, &mut rng);
        let children: Vec<&Strategy> = league
            .strategies
            .iter()
            .filter(|s| born.contains(&s.username))
            .collect();
        let targets: Vec<Option<String>> = children.iter().map(|s| target_of(&s.kind)).collect();

        assert_eq!(
            targets,
            vec![
                Some("science".into()),
                Some("culture".into()),
                Some("religious".into())
            ]
        );
        let science_child = children
            .iter()
            .find(|s| target_of(&s.kind).as_deref() == Some("science"))
            .unwrap();
        assert!(science_child
            .parents
            .contains(&"retired-science".to_string()));

        league.round = 4;
        let _ = evolve_league(&mut league, &cfg, &mut rng);
        let active_targets: std::collections::BTreeSet<String> = league
            .active()
            .into_iter()
            .filter_map(|i| target_of(&league.strategies[i].kind))
            .collect();
        assert!(VictoryTarget::ALL
            .iter()
            .all(|target| active_targets.contains(target.as_str())));
    }

    #[test]
    fn retirement_preserves_the_conservative_elite_in_each_niche() {
        let advanced = |target: Option<&str>| StrategyKind::Advanced {
            weights: Weights::default(),
            target: target.map(str::to_string),
        };
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("science-elite", advanced(Some("science")), 0),
                Strategy::new("science-duplicate", advanced(Some("science")), 0),
                Strategy::new("generalist-elite", advanced(None), 0),
                Strategy::new(
                    "reference",
                    StrategyKind::Builtin {
                        ai: "random".into(),
                    },
                    0,
                ),
            ],
            calibration: Calibration::default(),
        };
        for strategy in &mut league.strategies {
            strategy.games = MIN_GAMES_TO_RETIRE;
            strategy.rd = 30.0;
        }
        league.strategies[0].rating = 1650.0;
        league.strategies[1].rating = 1100.0;
        league.strategies[2].rating = 1550.0;
        league.strategies[3].rating = 1200.0;
        league.strategies[3].anchor = true;
        ensure_usernames(&mut league);

        let cfg = LeagueCfg {
            max_pop: 4,
            ..LeagueCfg::default()
        };
        let mut rng = Rng::new(32);
        let (_, retired) = evolve_league(&mut league, &cfg, &mut rng);
        let retired_names: Vec<&str> = league
            .strategies
            .iter()
            .filter(|s| retired.contains(&s.username))
            .map(|s| s.name.as_str())
            .collect();

        assert_eq!(retired_names, vec!["science-duplicate"]);
        assert!(!league.strategies[0].retired);
    }

    #[test]
    fn retirement_uses_the_lowest_upper_confidence_bound() {
        let mut league = League {
            round: 4,
            strategies: Vec::new(),
            calibration: Calibration::default(),
        };
        for (name, rating, rd, anchor) in [
            ("raw-low-but-uncertain", 1400.0, 100.0, false),
            ("confidently-low", 1450.0, 20.0, false),
            ("reference", 1700.0, 30.0, true),
        ] {
            let mut s = Strategy::new(
                name,
                StrategyKind::Builtin {
                    ai: "random".into(),
                },
                0,
            );
            s.rating = rating;
            s.rd = rd;
            s.games = MIN_GAMES_TO_RETIRE;
            s.anchor = anchor;
            league.strategies.push(s);
        }
        ensure_usernames(&mut league);
        let cfg = LeagueCfg {
            max_pop: 2,
            ..LeagueCfg::default()
        };
        let mut rng = Rng::new(4);
        let (_, retired) = evolve_league(&mut league, &cfg, &mut rng);
        let retired_names: Vec<&str> = league
            .strategies
            .iter()
            .filter(|s| retired.contains(&s.username))
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(retired_names, vec!["confidently-low"]);
    }

    /// Usernames are themed to the lane, unique, stable for founders, and
    /// deterministically backfilled onto rosters saved before the field
    /// existed (the same league always regrows the same handles).
    #[test]
    fn usernames_are_themed_unique_and_deterministic() {
        let mut league = League {
            round: 0,
            strategies: vec![
                Strategy::new("advanced", StrategyKind::Builtin { ai: "advanced".into() }, 0),
                Strategy::new(
                    "adv-science",
                    StrategyKind::Advanced {
                        weights: Weights::default(),
                        target: Some("science".into()),
                    },
                    0,
                ),
                Strategy::new(
                    "g4-9",
                    StrategyKind::Advanced {
                        weights: Weights::default(),
                        target: Some("science".into()),
                    },
                    4,
                ),
                Strategy::new(
                    "g4-10",
                    StrategyKind::Advanced {
                        weights: Weights::default(),
                        target: Some("domination".into()),
                    },
                    4,
                ),
            ],
            calibration: Calibration::default(),
        };
        ensure_usernames(&mut league);
        assert_eq!(league.strategies[0].username, "JackOfAllTrades");
        assert_eq!(league.strategies[1].username, "TechPriest");
        assert!(username_pool(Some("science"))
            .iter()
            .any(|p| league.strategies[2].username.starts_with(p)));
        assert!(username_pool(Some("domination"))
            .iter()
            .any(|p| league.strategies[3].username.starts_with(p)));
        let handles: std::collections::BTreeSet<&String> =
            league.strategies.iter().map(|s| &s.username).collect();
        assert_eq!(handles.len(), league.strategies.len(), "handle collision");
        // backfill is a pure function of names: rerunning changes nothing
        let before: Vec<String> = league.strategies.iter().map(|s| s.username.clone()).collect();
        ensure_usernames(&mut league);
        let after: Vec<String> = league.strategies.iter().map(|s| s.username.clone()).collect();
        assert_eq!(before, after);
        // the leaderboard lists every player's handle with elo next to it
        let table = standings(&league);
        assert!(table.contains("TechPriest"));
        assert!(table.contains("1500 elo"));
    }

    /// Same seed, fresh dirs -> byte-identical league state, so `--jobs`
    /// and reruns cannot change ratings.
    #[test]
    fn league_runs_are_deterministic() {
        let base = std::env::temp_dir().join(format!("civvis-league-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let run = |sub: &str, jobs: usize| {
            let cfg = LeagueCfg {
                rounds: 2,
                games_per_round: 3,
                players_per_game: 2,
                width: 20,
                height: 14,
                max_turns: 25,
                num_city_states: 1,
                seed: 11,
                jobs,
                dir: base.join(sub).to_string_lossy().into_owned(),
                evolve_every: 2,
                max_pop: 6,
                verbose: false,
                worker_id: "determinism-test".to_string(),
                lease_seconds: 5,
            };
            let league = run_league(&cfg);
            serde_json::to_string(&league).unwrap()
        };
        let a = run("a", 1);
        let b = run("b", 4);
        assert_eq!(a, b);
        let _ = fs::remove_dir_all(&base);
    }

    fn publish_test_result(
        cfg: &LeagueCfg,
        manifest: &RoundManifest,
        job: &WorkJob,
        worker: &str,
    ) {
        atomic_write_json(
            &result_path(&cfg.dir, manifest.round, &job.id),
            &StoredOutcome {
                schema_version: WORK_SCHEMA_VERSION,
                engine: manifest.engine.clone(),
                worker: worker.to_string(),
                round: manifest.round,
                job_id: job.id.clone(),
                placements: job.table.clone(),
                leaders: vec!["Trajan".into(), "Cleopatra".into()],
                civs: vec!["Rome".into(), "Egypt".into()],
                ranks: vec![0, 1],
                won: vec![true, false],
                seed: job.seed,
                turn: 80,
                victory: "science".into(),
            },
        )
        .unwrap();
    }

    /// A supervisor normally restarts with the same stable worker ID. If that
    /// worker died after filling its fair share, its expired claims must not
    /// keep the replacement at a zero-claim quota forever (issue #118).
    #[test]
    fn restarted_worker_reclaims_its_own_stale_claim_at_a_full_quota() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-stale-worker-restart-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let cfg = LeagueCfg {
            games_per_round: 4,
            players_per_game: 2,
            jobs: 4,
            dir: dir.to_string_lossy().into_owned(),
            verbose: false,
            worker_id: "stable-worker".into(),
            lease_seconds: 60,
            ..LeagueCfg::default()
        };
        let league = seed_league(&cfg.dir);
        let manifest = load_or_create_manifest(&league, &cfg).unwrap();
        assert_eq!(manifest.jobs.len(), 4);
        for job in manifest.jobs.iter().skip(1) {
            publish_test_result(&cfg, &manifest, job, &cfg.worker_id);
        }

        let abandoned = &manifest.jobs[0];
        let path = claim_path(&cfg.dir, manifest.round, &abandoned.id);
        let old_lease = LeaseRecord {
            worker: cfg.worker_id.clone(),
            process: u32::MAX,
            created_unix: 1,
        };
        atomic_write_json(&path, &old_lease).unwrap();
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH))
            .unwrap();
        assert!(lease_is_stale(&path, cfg.lease_seconds));

        let claimed = claim_jobs(&cfg, &manifest).unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].job.id, abandoned.id);
        assert_ne!(claimed[0].lease, old_lease);
        assert_eq!(read_json::<LeaseRecord>(&path).unwrap(), claimed[0].lease);
        release_claim(&cfg, manifest.round, &claimed[0]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Expiry is the distinction: a live lease remains work in progress and
    /// must still consume this worker's fair share while a peer is active.
    #[test]
    fn live_claims_still_count_toward_a_shared_workers_fair_share() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-live-worker-share-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let cfg = LeagueCfg {
            games_per_round: 8,
            players_per_game: 2,
            jobs: 8,
            dir: dir.to_string_lossy().into_owned(),
            verbose: false,
            worker_id: "machine-a".into(),
            lease_seconds: 60,
            ..LeagueCfg::default()
        };
        let league = seed_league(&cfg.dir);
        let manifest = load_or_create_manifest(&league, &cfg).unwrap();
        assert_eq!(manifest.jobs.len(), 8);
        let _first_presence = register_worker(&cfg).unwrap();
        let mut peer_cfg = cfg.clone();
        peer_cfg.worker_id = "machine-b".into();
        let _peer_presence = register_worker(&peer_cfg).unwrap();
        for job in manifest.jobs.iter().take(2) {
            publish_test_result(&cfg, &manifest, job, &cfg.worker_id);
        }
        for job in manifest.jobs.iter().skip(2).take(2) {
            atomic_write_json(
                &claim_path(&cfg.dir, manifest.round, &job.id),
                &LeaseRecord {
                    worker: cfg.worker_id.clone(),
                    process: std::process::id(),
                    created_unix: unix_now(),
                },
            )
            .unwrap();
        }
        // Publishing is durable before claim cleanup. A crash in that small
        // window leaves both files, but the finished job is one contribution,
        // not a result plus a second leased job.
        atomic_write_json(
            &claim_path(&cfg.dir, manifest.round, &manifest.jobs[0].id),
            &LeaseRecord {
                worker: cfg.worker_id.clone(),
                process: std::process::id(),
                created_unix: unix_now(),
            },
        )
        .unwrap();

        assert_eq!(active_worker_count(&cfg.dir), 2);
        assert_eq!(worker_round_contributions(&cfg, &manifest), 4);
        assert!(claim_jobs(&cfg, &manifest).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_workers_publish_each_job_and_finalize_the_round_once() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-shared-league-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let cfg = LeagueCfg {
            rounds: 1,
            games_per_round: 4,
            players_per_game: 2,
            width: 20,
            height: 14,
            max_turns: 20,
            num_city_states: 1,
            seed: 91,
            jobs: 1,
            dir: dir.to_string_lossy().into_owned(),
            evolve_every: 0,
            max_pop: 12,
            verbose: false,
            worker_id: "coordinator".into(),
            lease_seconds: 5,
        };
        let manifest = {
            let _lock = acquire_league_lock(&cfg.dir, &cfg.worker_id).unwrap();
            let league = seed_league(&cfg.dir);
            load_or_create_manifest(&league, &cfg).unwrap()
        };
        let rendezvous = std::sync::Arc::new(std::sync::Barrier::new(3));
        let run_worker = |worker: &str| {
            let mut worker_cfg = cfg.clone();
            worker_cfg.worker_id = worker.to_string();
            let worker_manifest = manifest.clone();
            let rendezvous = rendezvous.clone();
            std::thread::spawn(move || {
                let _presence = register_worker(&worker_cfg).unwrap();
                rendezvous.wait();
                loop {
                    let claimed = claim_jobs(&worker_cfg, &worker_manifest).unwrap();
                    if claimed.is_empty() {
                        break;
                    }
                    execute_claims(&worker_cfg, &worker_manifest, &claimed).unwrap();
                }
                rendezvous.wait();
            })
        };
        let first = run_worker("machine-a");
        let second = run_worker("machine-b");
        rendezvous.wait();
        rendezvous.wait();
        first.join().unwrap();
        second.join().unwrap();

        let finalized = try_finalize_round(&cfg, &manifest).unwrap().unwrap();
        assert_eq!(finalized.league.round, 1);
        assert_eq!(
            finalized
                .league
                .strategies
                .iter()
                .map(|strategy| strategy.games)
                .sum::<u32>(),
            (manifest.jobs.len() * manifest.config.players_per_game) as u32,
        );
        assert!(try_finalize_round(&cfg, &manifest).unwrap().is_none());
        let result_count = fs::read_dir(round_dir(&cfg.dir, 0).join("results"))
            .unwrap()
            .count();
        assert_eq!(result_count, manifest.jobs.len());
        let contributors: BTreeSet<String> = manifest
            .jobs
            .iter()
            .map(|job| {
                read_json::<StoredOutcome>(&result_path(&cfg.dir, 0, &job.id))
                    .unwrap()
                    .worker
            })
            .collect();
        assert_eq!(
            contributors,
            BTreeSet::from(["machine-a".into(), "machine-b".into()])
        );
        let remaining_claims = fs::read_dir(round_dir(&cfg.dir, 0).join("claims"))
            .unwrap()
            .count();
        assert_eq!(remaining_claims, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
