//! Elo tournament harness: evaluate AI strategies against each other.
//!
//! The primary rating belongs to a player (a human account or named AI
//! strategy) and follows it across every leader/civilization draw. Separate
//! `(player, leader, civilization)` rows retain matchup diagnostics; leader
//! and civilization are both needed because they are not one-to-one (Eleanor,
//! for example, can lead either England or France). Multiplayer games are
//! scored as simultaneous pairwise results with `K/(n-1)` scaling.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi, Weights};
use crate::game::{default_speed, Action, Game, GameOptions, VictoryConditions};
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::{MapPoles, MapScript, MapSize, MapTopology};

pub const BUILTIN_AIS: [&str; 10] = [
    "advanced",
    "advanced_evolved",
    "advanced_v1",
    "basic",
    "random",
    "evolved",
    "neural",
    "strategic",
    "strategic_deep",
    "policy",
];

/// Controls intended for paired evaluator experiments, not persistent
/// tournament ratings. Keeping them out of `BUILTIN_AIS` prevents a control
/// factory from being pooled into the same player/leader rating key as
/// its treatment.
pub const EVAL_ONLY_AIS: [&str; 78] = [
    "basic_evolved",
    "advanced_policy_live_control",
    "advanced_envoy_policy",
    "advanced_envoy_infrastructure",
    "advanced_envoy_priority",
    "advanced_envoy_economy",
    "advanced_strategic_commitment",
    "advanced_evolved_commitment",
    "advanced_congress_counter",
    "advanced_congress_votes",
    "advanced_congress_counter_hard",
    "advanced_banking_dedication",
    "advanced_blind_to_leaders",
    "advanced_rush",
    "advanced_rush_connected",
    "advanced_city_strategy",
    "advanced_city_strategy_emphasis",
    "advanced_city_strategy_roles",
    "advanced_city_strategy_roles_raw",
    "advanced_city_strategy_raw",
    "advanced_city_strategy_bastion_only",
    "advanced_city_strategy_breadbasket_only",
    "advanced_city_strategy_comparative_only",
    "advanced_city_strategy_pressure_only",
    "advanced_belief_pressure",
    "advanced_civ_blind",
    "advanced_counter_in_lane",
    "advanced_counter_stand_down",
    "advanced_early_score_alarm",
    "advanced_early_score_build",
    "advanced_evolved_blind",
    "advanced_settler_commit",
    "advanced_wide_opening",
    "advanced_expansion_payback",
    "advanced_late_expansion",
    "advanced_expansion_dispatch",
    "advanced_expansion_complete",
    "advanced_food_first",
    "advanced_measured_dedication",
    "advanced_lane_reachable",
    "advanced_parallel_settlers",
    "advanced_settler_first",
    "advanced_prophet_first",
    "advanced_league_top",
    "strategic_cheap",
    "advanced_relief_scoped",
    "advanced_joint_tactics",
    "strategic_score",
    "strategic_doctrine",
    "strategic_joint",
    "strategic_r20",
    "strategic_r10",
    "strategic_nodefer",
    "strategic_r20h20",
    "strategic_h80",
    "strategic_rot20",
    "strategic_rot10",
    "strategic_deep",
    "strategic_ultra",
    "strategic_deep_default",
    "strategic_deep_tempo",
    "strategic_deep_conversion",
    "strategic_deep_checkmate",
    "strategic_deep_expand",
    "strategic_deep_consolidate",
    "strategic_deep_militarize",
    "strategic_deep_league",
    "production",
    "production_net",
    "policy_wide",
    "policy_wide_frozen",
    "strategic_warm",
    "strategic_religion_expand",
    "strategic_cold",
    "strategic_noprophet",
    "strategic_deep_adaptive",
    "strategic_rivals",
    "strategic_deep_rivals",
];

/// Register a selectable arm once, under a typed identity.  The factory,
/// artifact resolver, provenance report, and evaluator-collapse guard all use
/// this identity rather than maintaining separate string matches.
macro_rules! define_arm_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        enum ArmKind {
            $($variant),+
        }

        impl ArmKind {
            fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }

            const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }
        }
    };
}

define_arm_kinds! {
    Advanced => "advanced",
    AdvancedBankingDedication => "advanced_banking_dedication",
    AdvancedBeliefPressure => "advanced_belief_pressure",
    AdvancedBlindToLeaders => "advanced_blind_to_leaders",
    AdvancedCityStrategy => "advanced_city_strategy",
    AdvancedCityStrategyBastionOnly => "advanced_city_strategy_bastion_only",
    AdvancedCityStrategyBreadbasketOnly => "advanced_city_strategy_breadbasket_only",
    AdvancedCityStrategyComparativeOnly => "advanced_city_strategy_comparative_only",
    AdvancedCityStrategyEmphasis => "advanced_city_strategy_emphasis",
    AdvancedCityStrategyPressureOnly => "advanced_city_strategy_pressure_only",
    AdvancedCityStrategyRaw => "advanced_city_strategy_raw",
    AdvancedCityStrategyRoles => "advanced_city_strategy_roles",
    AdvancedCityStrategyRolesRaw => "advanced_city_strategy_roles_raw",
    AdvancedCivBlind => "advanced_civ_blind",
    AdvancedCongressCounter => "advanced_congress_counter",
    AdvancedCongressCounterHard => "advanced_congress_counter_hard",
    AdvancedCongressVotes => "advanced_congress_votes",
    AdvancedCounterInLane => "advanced_counter_in_lane",
    AdvancedCounterStandDown => "advanced_counter_stand_down",
    AdvancedEarlyScoreAlarm => "advanced_early_score_alarm",
    AdvancedEarlyScoreBuild => "advanced_early_score_build",
    AdvancedEnvoyEconomy => "advanced_envoy_economy",
    AdvancedEnvoyInfrastructure => "advanced_envoy_infrastructure",
    AdvancedEnvoyPolicy => "advanced_envoy_policy",
    AdvancedEnvoyPriority => "advanced_envoy_priority",
    AdvancedEvolved => "advanced_evolved",
    AdvancedEvolvedBlind => "advanced_evolved_blind",
    AdvancedEvolvedCommitment => "advanced_evolved_commitment",
    AdvancedExpansionComplete => "advanced_expansion_complete",
    AdvancedExpansionDispatch => "advanced_expansion_dispatch",
    AdvancedExpansionPayback => "advanced_expansion_payback",
    AdvancedFoodFirst => "advanced_food_first",
    AdvancedJointTactics => "advanced_joint_tactics",
    AdvancedLaneReachable => "advanced_lane_reachable",
    AdvancedLateExpansion => "advanced_late_expansion",
    AdvancedLeagueTop => "advanced_league_top",
    AdvancedMeasuredDedication => "advanced_measured_dedication",
    AdvancedParallelSettlers => "advanced_parallel_settlers",
    AdvancedPolicyLiveControl => "advanced_policy_live_control",
    AdvancedProphetFirst => "advanced_prophet_first",
    AdvancedReliefScoped => "advanced_relief_scoped",
    AdvancedRush => "advanced_rush",
    AdvancedRushConnected => "advanced_rush_connected",
    AdvancedSettlerCommit => "advanced_settler_commit",
    AdvancedSettlerFirst => "advanced_settler_first",
    AdvancedStrategicCommitment => "advanced_strategic_commitment",
    AdvancedV1 => "advanced_v1",
    AdvancedWideOpening => "advanced_wide_opening",
    Basic => "basic",
    BasicEvolved => "basic_evolved",
    Evolved => "evolved",
    Neural => "neural",
    Policy => "policy",
    PolicyWide => "policy_wide",
    PolicyWideFrozen => "policy_wide_frozen",
    Production => "production",
    ProductionNet => "production_net",
    Random => "random",
    Strategic => "strategic",
    StrategicCheap => "strategic_cheap",
    StrategicCold => "strategic_cold",
    StrategicDeep => "strategic_deep",
    StrategicDeepAdaptive => "strategic_deep_adaptive",
    StrategicDeepCheckmate => "strategic_deep_checkmate",
    StrategicDeepConsolidate => "strategic_deep_consolidate",
    StrategicDeepConversion => "strategic_deep_conversion",
    StrategicDeepDefault => "strategic_deep_default",
    StrategicDeepExpand => "strategic_deep_expand",
    StrategicDeepLeague => "strategic_deep_league",
    StrategicDeepMilitarize => "strategic_deep_militarize",
    StrategicDeepRivals => "strategic_deep_rivals",
    StrategicDeepTempo => "strategic_deep_tempo",
    StrategicDoctrine => "strategic_doctrine",
    StrategicH80 => "strategic_h80",
    StrategicJoint => "strategic_joint",
    StrategicNodefer => "strategic_nodefer",
    StrategicNoprophet => "strategic_noprophet",
    StrategicR10 => "strategic_r10",
    StrategicR20 => "strategic_r20",
    StrategicR20H20 => "strategic_r20h20",
    StrategicReligionExpand => "strategic_religion_expand",
    StrategicRivals => "strategic_rivals",
    StrategicRot10 => "strategic_rot10",
    StrategicRot20 => "strategic_rot20",
    StrategicScore => "strategic_score",
    StrategicUltra => "strategic_ultra",
    StrategicWarm => "strategic_warm",
}

/// On-disk schema for the shared player/leader/civilization rating ledger.
pub const ELO_SCHEMA_VERSION: u32 = 3;
/// Version of the game/rating contract, independent of the JSON shape. Bump
/// this when rules, default setup, or scoring semantics change enough that an
/// Elo point no longer measures the same experiment.
pub const ELO_PROTOCOL_VERSION: u32 = 2;
pub const ELO_BASE_RATING: f64 = 1500.0;
pub const DEFAULT_RATINGS_PATH: &str = "data/elo_ratings.json";
/// Immutable protocol-v1 baseline retained for historical comparison after
/// the fog-honest city-pressure repair changed the shared legacy controller.
pub const HISTORICAL_V1_RATINGS_PATH: &str = "data/elo_ratings_v1.json";
const LEAGUE_SNAPSHOT_DIR: &str = "data/league";
const LEAGUE_SNAPSHOT_FILE: &str = "data/league/league.json";

/// Schema 3 existed before `setup_contract` was serialized. Those files were
/// all created under this exact lobby contract, so their migration value must
/// remain historical even after a future protocol deliberately changes the
/// live defaults.
const SCHEMA3_LEGACY_SETUP_CONTRACT: &str = "base=civ6;era=0;difficulty=prince;barbarians=true;disasters=2;modes=none;leader-pool=civ6;civilizations=stock-fill;randomize-civs=false;human-seats=none;teams=free-for-all;victories=science+culture+religious+diplomatic+domination+score";

fn schema3_legacy_setup_contract() -> String {
    SCHEMA3_LEGACY_SETUP_CONTRACT.to_string()
}

/// Outcome-affecting tournament settings that are fixed by the harness rather
/// than exposed in [`TourneyCfg`]. Derive the string from the same defaults
/// [`play_tournament`] constructs so changing one cannot silently append a
/// different experiment to an existing ledger.
fn tournament_setup_contract(cfg: &TourneyCfg) -> String {
    let options = GameOptions::new(
        cfg.players_per_game,
        cfg.width,
        cfg.height,
        0,
        cfg.max_turns,
        cfg.num_city_states,
    );
    let modes = if options.game_modes.is_empty() {
        "none".to_string()
    } else {
        options
            .game_modes
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("+")
    };
    let civilizations = if options.civs.is_empty() {
        "stock-fill".to_string()
    } else {
        options.civs.join("+")
    };
    let human_seats = if options.human_seats.is_empty() {
        "none".to_string()
    } else {
        options
            .human_seats
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("+")
    };
    let teams = if options.teams.is_empty() {
        "free-for-all".to_string()
    } else {
        options
            .teams
            .iter()
            .map(|team| team.map_or_else(|| "none".to_string(), |team| team.to_string()))
            .collect::<Vec<_>>()
            .join("+")
    };
    let victories = VictoryConditions::NAMES
        .into_iter()
        .filter(|victory| VictoryConditions::default().is_enabled(victory))
        .collect::<Vec<_>>()
        .join("+");
    format!(
        "base={};era={};difficulty={};barbarians={};disasters={};modes={};leader-pool={};civilizations={};randomize-civs={};human-seats={};teams={};victories={}",
        options.base_ruleset.id(),
        options.start_era,
        options.difficulty,
        options.barbarians,
        options.disaster_intensity,
        modes,
        options.leader_pool.id(),
        civilizations,
        options.randomize_civs,
        human_seats,
        teams,
        victories,
    )
}

/// Conservatively strongest *winning* active, untargeted genome in the
/// committed outcome-rated league. Lane specialists answer a different
/// question; this challenger isolates whether the same win-selected generalist
/// the evolutionary loop prefers transfers to the strongest macro-search
/// budget.
fn league_generalist() -> Option<(String, Weights)> {
    crate::league::load_league(LEAGUE_SNAPSHOT_DIR)?
        .strategies
        .into_iter()
        .filter(|strategy| !strategy.retired && !strategy.human)
        .filter_map(|strategy| {
            let win = crate::league::win_lower_confidence(&strategy);
            let rating = strategy.rating - 1.96 * strategy.rd;
            match strategy.kind {
                crate::league::StrategyKind::Advanced {
                    weights,
                    target: None,
                } => Some((win, rating, strategy.name, weights)),
                _ => None,
            }
        })
        .max_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then_with(|| right.2.cmp(&left.2))
        })
        .map(|(_, _, name, weights)| (name, weights))
}

/// The weights used by `advanced_league_top`.  Keep this extraction shared
/// with the evaluator spec so the reported source cannot diverge from the
/// controller the factory constructs.
fn shipped_league_top_advanced() -> Option<Weights> {
    let league = crate::league::shipped_league()?;
    let mut best: Option<(f64, Weights)> = None;
    for index in league.active() {
        let strategy = &league.strategies[index];
        if let crate::league::StrategyKind::Advanced { weights, .. } = &strategy.kind {
            if best
                .as_ref()
                .map(|(rating, _)| strategy.rating > *rating)
                .unwrap_or(true)
            {
                best = Some((strategy.rating, weights.clone()));
            }
        }
    }
    best.map(|(_, weights)| weights)
}

/// Resolve the leader supplied by the active ruleset. Keeping this beside the
/// ledger migration also gives old civilization-only rows an unambiguous home.
pub fn leader_for_civilization(civilization: &str) -> String {
    Rules::embedded()
        .civs
        .get(civilization)
        .map(|spec| spec.leader.clone())
        .unwrap_or_else(|| civilization.to_string())
}

pub fn expected(ra: f64, rb: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((rb - ra) / 400.0))
}

/// Each rating's chance of *winning outright* against the rest of the table,
/// summing to 1.
pub fn win_shares(ratings: &[f64]) -> Vec<f64> {
    let top = ratings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = ratings
        .iter()
        .map(|rating| 10f64.powf((rating - top) / 400.0))
        .collect();
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return vec![1.0 / ratings.len().max(1) as f64; ratings.len()];
    }
    weights.iter().map(|weight| weight / total).collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct RatingKey {
    pub player: String,
    pub leader: String,
    pub civilization: String,
}

impl RatingKey {
    pub fn new(
        player: impl Into<String>,
        leader: impl Into<String>,
        civilization: impl Into<String>,
    ) -> Self {
        Self {
            player: player.into(),
            leader: leader.into(),
            civilization: civilization.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub elo: f64,
    pub games: u32,
    pub wins: u32,
}

/// The game contract one persistent Elo ledger measures.
///
/// Ratings from different table sizes, maps, speeds, turn limits, or K factors
/// are different experiments. Older ledgers silently mixed them. Schema 3
/// binds the first run to its complete tournament profile and rejects later
/// incompatible runs, so a number can serve as a longitudinal baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TournamentProfile {
    pub protocol_version: u32,
    /// Fingerprint of the fully merged stock + mod rules JSON. Mod names are
    /// retained below for readability; this value binds their actual content.
    pub rules_fingerprint: String,
    /// Lobby settings fixed by the tournament harness rather than exposed
    /// in `TourneyCfg`. Older schema-3 ledgers deserialize to the historical
    /// defaults, then write this contract explicitly on their next checkpoint.
    #[serde(default = "schema3_legacy_setup_contract")]
    pub setup_contract: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_anchor: Option<String>,
    /// Ordered controller roles in the tournament. Immutable player identities
    /// may version between runs, but changing a role changes the multiplayer
    /// environment and therefore requires a different ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controller_roster: Vec<String>,
    pub players_per_game: usize,
    pub width: i32,
    pub height: i32,
    pub max_turns: u32,
    pub num_city_states: usize,
    pub speed: String,
    pub map_script: String,
    pub map_topology: String,
    pub map_poles: String,
    pub mods: Vec<String>,
    pub k: f64,
}

impl TournamentProfile {
    fn from_cfg(cfg: &TourneyCfg) -> Self {
        Self {
            protocol_version: ELO_PROTOCOL_VERSION,
            rules_fingerprint: Rules::embedded().source_fingerprint().to_string(),
            setup_contract: tournament_setup_contract(cfg),
            rating_anchor: cfg.rating_anchor.clone(),
            controller_roster: cfg.controller_roster.clone(),
            players_per_game: cfg.players_per_game,
            width: cfg.width,
            height: cfg.height,
            max_turns: cfg.max_turns,
            num_city_states: cfg.num_city_states,
            speed: cfg.speed.clone(),
            map_script: cfg.map_script.id().to_string(),
            map_topology: cfg.map_topology.id().to_string(),
            map_poles: cfg.map_poles.id().to_string(),
            mods: crate::mods::active_names(),
            k: cfg.k,
        }
    }

    fn validate(&self) -> bool {
        self.protocol_version > 0
            && self.rules_fingerprint.starts_with("fnv1a64:")
            && self.rules_fingerprint.len() == "fnv1a64:".len() + 16
            && self.rules_fingerprint["fnv1a64:".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && !self.setup_contract.trim().is_empty()
            && self
                .rating_anchor
                .as_ref()
                .is_none_or(|anchor| !anchor.trim().is_empty())
            && self.controller_roster.iter().all(|name| !name.trim().is_empty())
            && (self.controller_roster.is_empty()
                || (self.controller_roster.len() >= self.players_per_game
                    && self
                        .controller_roster
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        == self.controller_roster.len()))
            && (2..=100).contains(&self.players_per_game)
            && self.width >= 8
            && self.height >= 8
            && self.max_turns > 0
            && Rules::embedded().speeds.contains_key(&self.speed)
            && MapScript::from_id(&self.map_script).is_some()
            && MapTopology::from_id(&self.map_topology).is_some()
            && MapPoles::from_id(&self.map_poles).is_some()
            && self.mods.iter().all(|name| !name.trim().is_empty())
            && self.k.is_finite()
            && self.k > 0.0
    }

    pub fn label(&self) -> String {
        let mods = if self.mods.is_empty() {
            "stock".to_string()
        } else {
            self.mods.join("+")
        };
        let anchor = self.rating_anchor.as_deref().unwrap_or("floating");
        let controllers = if self.controller_roster.is_empty() {
            "unbound".to_string()
        } else {
            self.controller_roster.join(",")
        };
        format!(
            "protocol v{}, rules={}, setup={}, {}p {}x{}, {} turns, {} city-states, {}, {}/{}/{}, mods={}, K={}, anchor={}, controllers={}",
            self.protocol_version,
            self.rules_fingerprint,
            self.setup_contract,
            self.players_per_game,
            self.width,
            self.height,
            self.max_turns,
            self.num_city_states,
            self.speed,
            self.map_script,
            self.map_topology,
            self.map_poles,
            mods,
            self.k,
            anchor,
            controllers,
        )
    }
}

impl Rating {
    fn new(base: f64) -> Self {
        Self {
            elo: base,
            games: 0,
            wins: 0,
        }
    }
}

fn rating_maps_match<K: Ord>(left: &BTreeMap<K, Rating>, right: &BTreeMap<K, Rating>) -> bool {
    const ROUND_TRIP_TOLERANCE: f64 = 1e-9;
    left.len() == right.len()
        && left.iter().all(|(key, rating)| {
            right.get(key).is_some_and(|other| {
                rating.games == other.games
                    && rating.wins == other.wins
                    && (rating.elo - other.elo).abs() <= ROUND_TRIP_TOLERANCE
            })
        })
}

#[derive(Clone, Debug, PartialEq)]
pub struct EloPool {
    pub base_rating: f64,
    /// Profile-independent player summaries. These accumulate across every
    /// leader/civilization draw and provide the stable longitudinal baseline;
    /// exact combination rows below remain independently queryable.
    pub overall: BTreeMap<String, Rating>,
    /// The rating identity is deliberately structured, not a display string:
    /// player, leader, and civilization can be queried independently.
    pub ratings: BTreeMap<RatingKey, Rating>,
    /// Once present, every future tournament written to this ledger must match
    /// it exactly. `None` exists only for in-memory/manual pools and migrated
    /// schema-1/2 files until their first schema-3 tournament run.
    pub profile: Option<TournamentProfile>,
    /// Ordered raw game evidence. Fresh schema-3 ledgers can rebuild every
    /// aggregate from this log; migrated ledgers retain their old aggregate
    /// as an unauditable prior and mark the history incomplete.
    pub history: Vec<RatedGame>,
    pub history_complete: bool,
}

#[derive(Serialize, Deserialize)]
struct StoredPool {
    schema_version: u32,
    base_rating: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile: Option<TournamentProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    players: Vec<StoredPlayerRating>,
    ratings: Vec<StoredRating>,
    #[serde(default)]
    history_complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    games: Vec<RatedGame>,
}

#[derive(Serialize, Deserialize)]
struct StoredPlayerRating {
    player: String,
    elo: f64,
    games: u32,
    wins: u32,
}

#[derive(Serialize, Deserialize)]
struct StoredRating {
    #[serde(default)]
    player: String,
    #[serde(default)]
    leader: String,
    civilization: String,
    /// Schema-1 migration source. A legacy strategy becomes the player only
    /// when the row does not identify exactly one contributing AI factory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strategy: Option<String>,
    elo: f64,
    games: u32,
    wins: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    agents: Vec<String>,
}

/// Everything needed to score one rated major at the end of a game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatedPlayer {
    pub key: RatingKey,
    pub score: i64,
    pub won: bool,
}

/// One immutable rating event. Persistent tournament events carry a stable
/// id derived from the map seed and ordered entrant identities; manual library
/// callers leave it absent and retain insertion order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatedGame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub players: Vec<RatedPlayer>,
    pub k: f64,
}

impl RatedPlayer {
    pub fn new(
        player: impl Into<String>,
        leader: impl Into<String>,
        civilization: impl Into<String>,
        score: i64,
        won: bool,
    ) -> Self {
        Self {
            key: RatingKey::new(player, leader, civilization),
            score,
            won,
        }
    }
}

fn head_to_head_score(a: &RatedPlayer, b: &RatedPlayer) -> f64 {
    if a.won != b.won {
        f64::from(a.won)
    } else if a.score > b.score {
        1.0
    } else if a.score < b.score {
        0.0
    } else {
        0.5
    }
}

fn valid_rated_players(players: &[RatedPlayer], distinct_identities: bool) -> bool {
    players.len() >= 2
        && players.iter().all(|player| {
            !player.key.player.trim().is_empty()
                && !player.key.leader.trim().is_empty()
                && !player.key.civilization.trim().is_empty()
        })
        && (!distinct_identities
            || players
                .iter()
                .map(|player| player.key.player.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                == players.len())
}

impl EloPool {
    /// Keep the historical constructor shape for library callers. Entrants no
    /// longer create rating rows up front because their leader/civilization
    pub fn new(_names: &[String], base: f64) -> EloPool {
        EloPool {
            base_rating: base,
            overall: BTreeMap::new(),
            ratings: BTreeMap::new(),
            profile: None,
            history: Vec::new(),
            history_complete: true,
        }
    }

    pub fn with_base(base: f64) -> EloPool {
        Self::new(&[], base)
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<EloPool> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        let stored: StoredPool = serde_json::from_str(&raw).map_err(|error| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid Elo ledger {}: {error}", path.display()),
            )
        })?;
        if !matches!(stored.schema_version, 1 | 2 | ELO_SCHEMA_VERSION) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "unsupported Elo schema {} in {}; expected {}",
                    stored.schema_version,
                    path.display(),
                    ELO_SCHEMA_VERSION
                ),
            ));
        }
        if !stored.base_rating.is_finite() {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("non-finite base rating in {}", path.display()),
            ));
        }
        if stored
            .profile
            .as_ref()
            .is_some_and(|profile| !profile.validate())
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("invalid tournament profile in {}", path.display()),
            ));
        }
        let mut ratings: BTreeMap<RatingKey, Rating> = BTreeMap::new();
        for row in stored.ratings {
            let player = if stored.schema_version == 1 {
                if row.agents.len() == 1 {
                    row.agents[0].clone()
                } else {
                    row.strategy.clone().unwrap_or_default()
                }
            } else {
                row.player
            };
            let leader = if stored.schema_version == 1 {
                leader_for_civilization(&row.civilization)
            } else {
                row.leader
            };
            if player.trim().is_empty()
                || leader.trim().is_empty()
                || row.civilization.trim().is_empty()
                || !row.elo.is_finite()
                || row.wins > row.games
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid rating row in {}", path.display()),
                ));
            }
            let key = RatingKey::new(player, leader, row.civilization);
            let rating = Rating {
                elo: row.elo,
                games: row.games,
                wins: row.wins,
            };
            if let Some(existing) = ratings.get_mut(&key) {
                if stored.schema_version >= 2 {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "duplicate player/leader/civilization row {key:?} in {}",
                            path.display()
                        ),
                    ));
                }
                let total = existing.games.saturating_add(rating.games);
                if total > 0 {
                    existing.elo = (existing.elo * existing.games as f64
                        + rating.elo * rating.games as f64)
                        / total as f64;
                }
                existing.games = total;
                existing.wins = existing.wins.saturating_add(rating.wins);
            } else {
                ratings.insert(key, rating);
            }
        }
        let mut overall = BTreeMap::new();
        for row in stored.players {
            if row.player.trim().is_empty()
                || !row.elo.is_finite()
                || row.wins > row.games
                || overall.contains_key(&row.player)
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid overall player rating in {}", path.display()),
                ));
            }
            overall.insert(
                row.player,
                Rating {
                    elo: row.elo,
                    games: row.games,
                    wins: row.wins,
                },
            );
        }
        // Schema 1/2 had only combination rows. Preserve their scale and give
        // each player the games-weighted centre of those rows as a migration
        // prior. The old files cannot recover how many distinct worlds those
        // seats came from, so the global game/win counters start at zero;
        // exact combination rows retain all of the legacy counts.
        let mut accumulated = BTreeMap::<String, (f64, u32)>::new();
        for (key, rating) in &ratings {
            let entry = accumulated.entry(key.player.clone()).or_default();
            entry.0 += rating.elo * f64::from(rating.games);
            entry.1 = entry.1.saturating_add(rating.games);
        }
        for (player, (weighted, games)) in accumulated {
            if !overall.contains_key(&player) {
                overall.insert(
                    player,
                    Rating {
                        elo: if games > 0 {
                            weighted / f64::from(games)
                        } else {
                            stored.base_rating
                        },
                        games: 0,
                        wins: 0,
                    },
                );
            }
        }
        let games = if stored.schema_version == ELO_SCHEMA_VERSION {
            stored.games
        } else {
            Vec::new()
        };
        let mut event_ids = BTreeSet::new();
        let mut keyed_games = 0usize;
        for game in &games {
            let valid_id = match &game.id {
                Some(id) => {
                    keyed_games += 1;
                    !id.trim().is_empty() && event_ids.insert(id.clone())
                }
                None => true,
            };
            let valid_players = valid_rated_players(&game.players, game.id.is_some())
                && stored.profile.as_ref().is_none_or(|profile| {
                    game.players.len() == profile.players_per_game
                });
            if !valid_id || !valid_players || !game.k.is_finite() || game.k < 0.0 {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid raw game evidence in {}", path.display()),
                ));
            }
        }
        let mixed_keying = keyed_games != 0 && keyed_games != games.len();
        let unordered_keys =
            keyed_games != 0 && !games.windows(2).all(|pair| pair[0].id < pair[1].id);
        let profile_k_mismatch = stored.profile.as_ref().is_some_and(|profile| {
            games
                .iter()
                .any(|game| (game.k - profile.k).abs() > f64::EPSILON)
        });
        if mixed_keying || unordered_keys || profile_k_mismatch {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "non-canonical raw game history in {} (mixed keyed/unkeyed: {}, ordered: {}, profile K matches: {})",
                    path.display(),
                    mixed_keying,
                    !unordered_keys,
                    !profile_k_mismatch,
                ),
            ));
        }
        let history_complete =
            stored.schema_version == ELO_SCHEMA_VERSION && stored.history_complete;
        if history_complete {
            let mut replay = EloPool::with_base(stored.base_rating);
            replay.profile = stored.profile.clone();
            for game in &games {
                replay.apply_game(&game.players, game.k);
            }
            let players_match = rating_maps_match(&replay.overall, &overall);
            let combinations_match = rating_maps_match(&replay.ratings, &ratings);
            if !players_match || !combinations_match {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Elo aggregates do not match raw game evidence in {} (players match: {}, combinations match: {})",
                        path.display(),
                        players_match,
                        combinations_match,
                    ),
                ));
            }
        }
        Ok(EloPool {
            base_rating: stored.base_rating,
            overall,
            ratings,
            profile: stored.profile,
            history: games,
            history_complete,
        })
    }

    pub fn load_or_new(path: impl AsRef<Path>, base: f64) -> io::Result<EloPool> {
        match Self::load(path) {
            Ok(pool) => Ok(pool),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::with_base(base)),
            Err(error) => Err(error),
        }
    }

    /// Bind a persistent ledger to one reproducible tournament contract.
    ///
    /// A migrated schema-1/2 file has no profile, so its first schema-3 run
    /// records one. Once bound, mixing evidence from another profile is an
    /// error rather than a silent change in what one Elo point means.
    pub fn bind_profile(&mut self, profile: TournamentProfile) -> io::Result<()> {
        if !profile.validate() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid tournament rating profile",
            ));
        }
        if self
            .history
            .iter()
            .any(|game| {
                (game.k - profile.k).abs() > f64::EPSILON
                    || game.players.len() != profile.players_per_game
            })
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "raw game evidence does not match the tournament rating profile",
            ));
        }
        match &self.profile {
            Some(existing) if existing != &profile => Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "rating profile mismatch: ledger is [{}], requested [{}]; use a different --ratings path for a different experiment",
                    existing.label(),
                    profile.label(),
                ),
            )),
            Some(_) => Ok(()),
            None => {
                self.profile = Some(profile);
                Ok(())
            }
        }
    }

    /// Atomically replace a ledger, so interruption cannot leave partial JSON.
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let stored = StoredPool {
            schema_version: ELO_SCHEMA_VERSION,
            base_rating: self.base_rating,
            profile: self.profile.clone(),
            players: self
                .overall
                .iter()
                .map(|(player, rating)| StoredPlayerRating {
                    player: player.clone(),
                    elo: rating.elo,
                    games: rating.games,
                    wins: rating.wins,
                })
                .collect(),
            ratings: self
                .ratings
                .iter()
                .map(|(key, rating)| StoredRating {
                    player: key.player.clone(),
                    leader: key.leader.clone(),
                    civilization: key.civilization.clone(),
                    strategy: None,
                    elo: rating.elo,
                    games: rating.games,
                    wins: rating.wins,
                    agents: Vec::new(),
                })
                .collect(),
            history_complete: self.history_complete,
            games: self.history.clone(),
        };
        let mut raw = serde_json::to_vec_pretty(&stored).map_err(io::Error::other)?;
        raw.push(b'\n');

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("elo_ratings.json");
        let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(&raw)?;
            file.sync_all()?;
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Pairwise, simultaneous Elo update from the pre-game ratings. Equal
    /// scores are draws unless one player is the engine-declared winner.
    pub fn record_game(&mut self, players: &[RatedPlayer], k: f64) {
        if players.len() < 2 {
            return;
        }
        assert!(
            k.is_finite() && k >= 0.0,
            "Elo K must be finite and non-negative"
        );
        assert!(
            self.profile
                .as_ref()
                .is_none_or(|profile| (profile.k - k).abs() <= f64::EPSILON),
            "Elo K must match the bound tournament profile"
        );
        assert!(
            self.profile
                .as_ref()
                .is_none_or(|profile| players.len() == profile.players_per_game),
            "Elo table size must match the bound tournament profile"
        );
        self.apply_game(players, k);
        self.history.push(RatedGame {
            id: None,
            players: players.to_vec(),
            k,
        });
    }

    /// Insert a reproducibly identified tournament game exactly once.
    ///
    /// Keyed evidence is sorted before a full replay, so two concurrent
    /// tournament processes produce the same table regardless of which lock
    /// they acquire first. Repeating an identical run is idempotent; the same
    /// identity producing different evidence is rejected as a reproducibility
    /// failure instead of double-counted.
    pub fn record_game_once(
        &mut self,
        id: impl Into<String>,
        players: &[RatedPlayer],
        k: f64,
    ) -> io::Result<bool> {
        let id = id.into();
        if id.trim().is_empty()
            || !valid_rated_players(players, true)
            || !k.is_finite()
            || k < 0.0
            || self
                .profile
                .as_ref()
                .is_some_and(|profile| players.len() != profile.players_per_game)
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid keyed Elo game (table size and player identities must be distinct and match the profile)",
            ));
        }
        if self
            .profile
            .as_ref()
            .is_some_and(|profile| (profile.k - k).abs() > f64::EPSILON)
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Elo K does not match the bound tournament profile",
            ));
        }
        let candidate = RatedGame {
            id: Some(id.clone()),
            players: players.to_vec(),
            k,
        };
        if let Some(existing) = self
            .history
            .iter()
            .find(|game| game.id.as_deref() == Some(id.as_str()))
        {
            return if existing == &candidate {
                Ok(false)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Elo event {id:?} was replayed with different results; use a new versioned rating identity when a controller changes"
                    ),
                ))
            };
        }
        if self.history.iter().any(|game| game.id.is_none()) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "cannot mix reproducibly keyed tournament evidence with unkeyed manual games; use a different --ratings path",
            ));
        }
        let appends_in_order = self
            .history
            .last()
            .and_then(|game| game.id.as_deref())
            .is_none_or(|last| last < id.as_str());
        self.history.push(candidate);
        if self.history_complete {
            if appends_in_order {
                self.apply_game(players, k);
            } else {
                self.history.sort_by(|a, b| a.id.cmp(&b.id));
                let history = self.history.clone();
                self.overall.clear();
                self.ratings.clear();
                for game in &history {
                    self.apply_game(&game.players, game.k);
                }
                self.history = history;
            }
        } else {
            // A schema-1/2 aggregate has no recoverable raw starting evidence.
            // Its new events are still retained and deduplicated, but cannot
            // be reordered ahead of that imported prior.
            self.apply_game(players, k);
        }
        Ok(true)
    }

    fn apply_game(&mut self, players: &[RatedPlayer], k: f64) {
        let mut by_player = BTreeMap::<String, Vec<&RatedPlayer>>::new();
        for player in players {
            by_player
                .entry(player.key.player.clone())
                .or_default()
                .push(player);
            self.overall
                .entry(player.key.player.clone())
                .or_insert_with(|| Rating::new(self.base_rating));
        }
        for player in players {
            let prior = self.overall[&player.key.player].elo;
            self.ratings
                .entry(player.key.clone())
                .or_insert_with(|| Rating::new(prior));
        }

        // One global player identity accumulates across every civilization it
        // draws. When a tournament has fewer entrants than seats, average all
        // cross-seat comparisons for one player pair and count that pair once;
        // cloned seats are correlated and must not manufacture four games of
        // rating evidence from one world.
        let identities: Vec<String> = by_player.keys().cloned().collect();
        if identities.len() >= 2 {
            let scale = k / (identities.len() as f64 - 1.0);
            let mut overall_delta = BTreeMap::<String, f64>::new();
            for i in 0..identities.len() {
                for j in (i + 1)..identities.len() {
                    let a_name = &identities[i];
                    let b_name = &identities[j];
                    let mut actual = 0.0;
                    let mut comparisons = 0usize;
                    for a in &by_player[a_name] {
                        for b in &by_player[b_name] {
                            actual += head_to_head_score(a, b);
                            comparisons += 1;
                        }
                    }
                    actual /= comparisons.max(1) as f64;
                    let expectation =
                        expected(self.overall[a_name].elo, self.overall[b_name].elo);
                    let change = scale * (actual - expectation);
                    *overall_delta.entry(a_name.clone()).or_default() += change;
                    *overall_delta.entry(b_name.clone()).or_default() -= change;
                }
            }
            for (name, change) in overall_delta {
                self.overall.get_mut(&name).unwrap().elo += change;
            }
        }
        for (name, seats) in &by_player {
            let rating = self.overall.get_mut(name).unwrap();
            rating.games = rating.games.saturating_add(1);
            rating.wins = rating
                .wins
                .saturating_add(u32::from(seats.iter().any(|seat| seat.won)));
        }

        let scale = k / (players.len() as f64 - 1.0);
        let mut delta: BTreeMap<RatingKey, f64> = BTreeMap::new();
        for i in 0..players.len() {
            for j in (i + 1)..players.len() {
                let a = &players[i];
                let b = &players[j];
                if a.key.player == b.key.player {
                    // A tournament may reuse one AI player when there are
                    // fewer entrants than seats. Its leader ratings must not
                    // manufacture evidence by competing against themselves.
                    continue;
                }
                let actual_a = head_to_head_score(a, b);
                let elo_a = self.ratings[&a.key].elo;
                let elo_b = self.ratings[&b.key].elo;
                let change = scale * (actual_a - expected(elo_a, elo_b));
                *delta.entry(a.key.clone()).or_insert(0.0) += change;
                *delta.entry(b.key.clone()).or_insert(0.0) -= change;
            }
        }
        for (key, change) in delta {
            self.ratings.get_mut(&key).unwrap().elo += change;
        }
        for player in players {
            let rating = self.ratings.get_mut(&player.key).unwrap();
            rating.games = rating.games.saturating_add(1);
            rating.wins = rating.wins.saturating_add(u32::from(player.won));
        }
        self.recenter_to_anchor();
    }

    /// Keep one contract-pinned control at the base rating. Elo expectations depend
    /// only on differences, so translating every row preserves every update
    /// while preventing repeated introductions of fresh 1500-rated identities
    /// from inflating later generations relative to old, inactive ones.
    fn recenter_to_anchor(&mut self) {
        let Some(anchor) = self
            .profile
            .as_ref()
            .and_then(|profile| profile.rating_anchor.as_ref())
        else {
            return;
        };
        let Some(anchor_rating) = self.overall.get(anchor).map(|rating| rating.elo) else {
            return;
        };
        let shift = self.base_rating - anchor_rating;
        for rating in self.overall.values_mut() {
            rating.elo += shift;
        }
        for rating in self.ratings.values_mut() {
            rating.elo += shift;
        }
    }

    /// Compatibility helper for callers with only a strict placement list.
    /// New evaluation code should use [`EloPool::record_game`] so it can retain
    /// civilization identity and score ties correctly.
    pub fn record(&mut self, placements: &[String], k: f64) {
        let players: Vec<RatedPlayer> = placements
            .iter()
            .enumerate()
            .map(|(place, name)| RatedPlayer {
                key: RatingKey::new(name, "unknown", "unknown"),
                score: (placements.len() - place) as i64,
                won: place == 0,
            })
            .collect();
        self.record_game(&players, k);
    }
}

/// Canonical controller identity for names whose artifacts can make them an
/// exact alias of another selectable agent. The resolved [`ArmKind`] is what
/// construction, provenance, and comparison all receive; a label cannot
/// independently describe a controller that the factory did not build.
fn artifact_effective_alias_from(
    kind: ArmKind,
    champion: bool,
    net: bool,
    wide_net: bool,
    league: bool,
) -> ArmKind {
    let basic_fallback = if champion {
        ArmKind::BasicEvolved
    } else {
        ArmKind::Basic
    };
    let advanced_fallback = if champion {
        ArmKind::AdvancedEvolved
    } else {
        ArmKind::Advanced
    };
    match kind {
        ArmKind::Evolved | ArmKind::AdvancedEvolved => advanced_fallback,
        ArmKind::BasicEvolved => basic_fallback,
        ArmKind::Neural => {
            if net {
                ArmKind::Neural
            } else {
                basic_fallback
            }
        }
        ArmKind::Policy => {
            if net {
                ArmKind::Policy
            } else {
                advanced_fallback
            }
        }
        ArmKind::PolicyWide => {
            if wide_net {
                ArmKind::PolicyWide
            } else {
                advanced_fallback
            }
        }
        ArmKind::PolicyWideFrozen => {
            if wide_net {
                ArmKind::PolicyWideFrozen
            } else {
                advanced_fallback
            }
        }
        ArmKind::Strategic | ArmKind::StrategicWarm => {
            if net {
                ArmKind::Strategic
            } else {
                ArmKind::StrategicScore
            }
        }
        ArmKind::ProductionNet => {
            if net {
                ArmKind::ProductionNet
            } else {
                ArmKind::Production
            }
        }
        ArmKind::StrategicDeepLeague => {
            if league {
                ArmKind::StrategicDeepLeague
            } else {
                ArmKind::StrategicDeep
            }
        }
        ArmKind::AdvancedEvolvedCommitment => {
            if champion {
                ArmKind::AdvancedEvolvedCommitment
            } else {
                ArmKind::AdvancedStrategicCommitment
            }
        }
        ArmKind::AdvancedEvolvedBlind => {
            if champion {
                ArmKind::AdvancedEvolvedBlind
            } else {
                ArmKind::AdvancedBlindToLeaders
            }
        }
        ArmKind::AdvancedBankingDedication => advanced_fallback,
        _ => kind,
    }
}

fn artifact_effective_alias(kind: ArmKind, dir: &str) -> ArmKind {
    // The hot factory path resolves only artifacts which can change its
    // canonical controller.  In particular, scripted `advanced` used to pay
    // a champion parse solely because evaluator metadata was bolted onto
    // construction; ordinary games must not do evaluator work per seat.
    let champion = matches!(
        kind,
        ArmKind::Evolved
            | ArmKind::AdvancedEvolved
            | ArmKind::BasicEvolved
            | ArmKind::Neural
            | ArmKind::Policy
            | ArmKind::PolicyWide
            | ArmKind::PolicyWideFrozen
            | ArmKind::AdvancedEvolvedCommitment
            | ArmKind::AdvancedEvolvedBlind
            | ArmKind::AdvancedBankingDedication
    ) && crate::evolve::load_champion(dir).is_some();
    let net = matches!(
        kind,
        ArmKind::Neural
            | ArmKind::Policy
            | ArmKind::Strategic
            | ArmKind::StrategicWarm
            | ArmKind::ProductionNet
    ) && crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let wide_net = matches!(kind, ArmKind::PolicyWide | ArmKind::PolicyWideFrozen)
        && crate::valuenet::ValueNet::load_width(dir, crate::decision_features::WIDTH).is_some();
    let league = kind == ArmKind::StrategicDeepLeague && league_generalist().is_some();
    artifact_effective_alias_from(kind, champion, net, wide_net, league)
}

/// Construct a canonical, already-resolved arm. Public callers enter through
/// [`builtin_arm`] or [`builtin_ai`], never by selecting a raw string here.
fn build_arm(kind: ArmKind, seed: u64) -> Box<dyn Ai> {
    match kind.name() {
        "advanced" => Box::new(AdvancedAi::new()),
        // The first bounded use of the fog-safe belief surface. It retains
        // only last-seen military strength for the already fog-filtered
        // city-pressure/recovery path; every other Advanced policy remains
        // unchanged, so this is an evaluator arm rather than a fog-honesty
        // claim for the whole controller.
        "advanced_belief_pressure" => {
            let mut ai = AdvancedAi::new();
            ai.belief_pressure = true;
            Box::new(ai)
        }
        // Four arms decompose the envoy-acquisition treatment. The live
        // policy control differs from `advanced` only by enabling the existing
        // counterfactual deck; the policy treatment then adds only influence
        // to that deck's valuation. Infrastructure remains on the stock deck,
        // and the combined arm carries both mechanisms.
        "advanced_policy_live_control" => {
            let mut weights = Weights::default();
            weights.policy_deck = crate::ai::PolicyDeck::Live;
            Box::new(AdvancedAi::with_weights(weights))
        }
        "advanced_envoy_policy" => {
            let mut weights = Weights::default();
            weights.policy_deck = crate::ai::PolicyDeck::Live;
            weights.pol_influence = 4.0;
            Box::new(AdvancedAi::with_weights(weights))
        }
        "advanced_envoy_infrastructure" => {
            let mut ai = AdvancedAi::new();
            ai.envoy_infrastructure = true;
            Box::new(ai)
        }
        // The valuation-only treatment above is routed around by ordinary
        // adaptive production. This arm keeps that valuation and additionally
        // reserves one empty city for the first legal, horizon-positive stage
        // of the Diplomatic Quarter -> Consulate -> Chancery chain.
        "advanced_envoy_priority" => {
            let mut ai = AdvancedAi::new();
            ai.envoy_infrastructure = true;
            ai.envoy_priority = true;
            Box::new(ai)
        }
        "advanced_envoy_economy" => {
            let mut weights = Weights::default();
            weights.policy_deck = crate::ai::PolicyDeck::Live;
            weights.pol_influence = 4.0;
            let mut ai = AdvancedAi::with_weights(weights);
            ai.envoy_infrastructure = true;
            Box::new(ai)
        }
        "advanced_strategic_commitment" => {
            let mut ai = AdvancedAi::new();
            ai.strategic_commitment = true;
            Box::new(ai)
        }
        // Composite of the strongest committed compact-profile genome and the
        // independently causal strategy-stability treatment. Its artifact is
        // definitional, so evaluators can refuse a silent stock fallback. The
        // preregistered 20-map matrix rejected transfer: compact was +17, but
        // deployment was -70 with terminal direction 5-15 (p=0.0414).
        "advanced_evolved_commitment" => {
            let mut ai = crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_else(AdvancedAi::new);
            ai.strategic_commitment = true;
            Box::new(ai)
        }
        // Treatment for the lane-reachability axis: identical to `advanced`
        // except that it refuses to route toward a victory lane it cannot
        // finish inside the turn budget. Paired against `advanced` this
        // isolates the filter and nothing else. Measured no stronger at 120
        // mirrored maps -- 49.6% paired score, Elo-equivalent -3, sign
        // p=1.0000, gate INCONCLUSIVE -- which is why it is an entrant and
        // not the default.
        // Ablation for the civilization-aware decision layer: identical to
        // `advanced` except that it ignores every by-name civilization signal
        // (the Greece and China lane floors, the unique-unit tech bonus, the
        // Egypt/China wonder exemption). It still builds whatever uniques it
        // has -- that is mechanics. Paired against `advanced` this bounds what
        // the existing per-civilization code is worth, which is the ceiling
        // any better per-civilization play has to beat. See `docs/OPENINGS.md`.
        // Treatment for the expansion-tempo axis: identical to `advanced`
        // except that its governors want food while the empire is short of
        // its city target. See `docs/OPENINGS.md` §11 for the ceiling that
        // motivated it and for the production it trades away.
        "advanced_food_first" => {
            let mut ai = AdvancedAi::new();
            ai.food_first = 0.6;
            Box::new(ai)
        }
        // Treatment for the settler-commitment axis: identical to `advanced`
        // except that a settler holds its chosen site across a turn it could
        // not move, for up to three such turns. See `docs/OPENINGS.md` §15.
        "advanced_settler_commit" => {
            let mut ai = AdvancedAi::new();
            ai.settler_commit = true;
            Box::new(ai)
        }
        // Ablation for the counter-leader axis: identical to `advanced`
        // except that it never reacts to a rival closing on a victory --
        // `victory_denial` is silent and `urgent_victory_threat` never waives
        // the ordinary war-readiness checks. It still fights, expands and
        // races; it just never does any of it *because* somebody else is
        // about to win. Paired against `advanced` this is what the whole
        // denial response is worth. See `docs/COUNTERING_LEADERS.md`, which
        // measures the layer as a near-perfect predictor of the winner, no
        // deterrent, and a real cost in development on its recorded large
        // profile.
        "advanced_blind_to_leaders" => {
            let mut ai = AdvancedAi::new();
            ai.deny_leaders = false;
            Box::new(ai)
        }
        // Treatment for the early-aggression axis: identical to `advanced`
        // except that it will open an ancient rush on the nearest weak
        // neighbour before anybody can build walls. `rush_census` measures the
        // window this plays in — 0% of capitals walled through turn 60, mean
        // garrison 0.7, nearest rival capital a median 13 tiles — and a Monte
        // Carlo over the engine's own combat formulas sizes the stack at four
        // melee units. `advanced` cannot reach any of it: `assess` withholds
        // Conquest until turn 55, and the declaration carries a turn-35 floor.
        // Paired against `advanced` this is what early aggression is worth.
        "advanced_rush" => {
            let mut ai = AdvancedAi::new();
            ai.early_rush = true;
            Box::new(ai)
        }
        // Frozen route selector for the ancient-rush mechanism. It changes no
        // rush rule after target eligibility: only rivals a starting land
        // melee unit could route to the existing staging ring may trigger it.
        "advanced_rush_connected" => {
            let mut ai = AdvancedAi::new();
            ai.early_rush = true;
            ai.route_connected_rush = true;
            Box::new(ai)
        }
        // Treatment for the response-shape axis: identical to `advanced`
        // except that a Science or Expansion threat is answered by racing the
        // leader in that lane rather than by declaring on them. The alarm is
        // unchanged; only what it asks for changes. See
        // `docs/COUNTERING_LEADERS.md`: on its recorded large profile, one or
        // two belligerents wins 4.4% and 10.7% of seats against a 16.7% base.
        "advanced_counter_in_lane" => {
            let mut ai = AdvancedAi::new();
            ai.counter_in_lane = true;
            Box::new(ai)
        }
        // Decomposition arm for the response-shape axis: reacts to the other
        // four races exactly as `advanced` does and to a Science or Expansion
        // threat not at all. Read against `advanced_counter_in_lane` it says
        // whether that treatment's effect is "stop declaring" or "race them".
        "advanced_counter_stand_down" => {
            let mut ai = AdvancedAi::new();
            ai.counter_stand_down = true;
            Box::new(ai)
        }
        // Instrument treatment: reads the score race as a margin over the
        // field instead of as a last-quarter clock. The response is unchanged.
        "advanced_early_score_alarm" => {
            let mut ai = AdvancedAi::new();
            ai.early_score_alarm = true;
            Box::new(ai)
        }
        // Treatment for the free-counter axis: identical to `advanced` except
        // that the World Congress resolutions carrying a targeted penalty --
        // `trade_policy` B (trade embargo), `migration_treaty` B (-20% growth),
        // `border_control_treaty` B (no border-growth annexation) -- aim at the
        // empire `victory_denial` names instead of at the empire holding the
        // most Diplomatic Victory Points. `congress_census` measures that
        // shipped target as the eventual winner 24.8% of the time at 4p
        // (base 25.0%) and 14.4% at the recorded 6p profile (base 16.7%),
        // against 61% for the score leader on both. Unlike every arm in
        // `docs/COUNTERING_LEADERS.md`, this response is not paid for in
        // development: `resolve_congress` refunds a losing vote in full.
        "advanced_congress_counter" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_leader = true;
            Box::new(ai)
        }
        // Decomposition arm: aims exactly where `advanced` aims and only
        // changes how hard it pushes, buying the second and third vote behind
        // a ballot that opposes the empire closest to a victory. Read against
        // `advanced_congress_counter` it separates the target from the weight.
        "advanced_congress_votes" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_votes = true;
            Box::new(ai)
        }
        // Both halves at once. Only informative once the two arms above have
        // been read separately.
        "advanced_congress_counter_hard" => {
            let mut ai = AdvancedAi::new();
            ai.congress_counter_leader = true;
            ai.congress_counter_votes = true;
            Box::new(ai)
        }
        // The earlier alarm asking for a build instead of a war -- the only
        // combination `docs/COUNTERING_LEADERS.md` leaves untested, since every
        // response-side change measured null on the shipped instrument.
        "advanced_early_score_build" => {
            let mut ai = AdvancedAi::new();
            ai.early_score_alarm = true;
            ai.counter_in_lane = true;
            Box::new(ai)
        }
        "advanced_civ_blind" => {
            let mut ai = AdvancedAi::new();
            ai.civ_blind = true;
            Box::new(ai)
        }
        // Treatment for the expansion-rate axis: identical to `advanced`
        // except that it may hold more than one settler at a time, up to its
        // shortfall against the city target. Paired against `advanced` this
        // isolates the empire-wide `counts.settlers == 0` serialization and
        // nothing else. See `docs/OPENINGS.md` for the measurement that
        // motivated it and for what would refute it.
        "advanced_parallel_settlers" => {
            let mut ai = AdvancedAi::new();
            ai.parallel_settlers = true;
            Box::new(ai)
        }
        // Treatment for the city-decision axis: identical to `advanced`
        // except that it stamps a `CityDirective` on every city each turn, so
        // the citizen governor can see the empire's lane, the city's own role
        // and the hostile strength standing next to it. Paired against
        // `advanced` this isolates the plan-to-tile channel and nothing else.
        // See `AdvancedAi::city_strategy`.
        "advanced_city_strategy" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            Box::new(ai)
        }
        // The two ablation halves of `advanced_city_strategy`, which lost its
        // first screen at 42.5% over 120 maps. Emphasis-only carries the
        // empire's lane into the tiles and nothing else; roles-only carries
        // the local role ladder and per-city military pressure and nothing
        // else. Paired against `advanced` they attribute that loss instead of
        // leaving it to be guessed at.
        "advanced_city_strategy_emphasis" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_roles = false;
            Box::new(ai)
        }
        "advanced_city_strategy_roles" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            Box::new(ai)
        }
        // The frozen pre-repair controls: the role ladder allowed to type a
        // city comparatively while the empire is still expanding, which is
        // what the 42.1% and 42.5% results were measured on. Kept so those
        // numbers stay reproducible now that the repair is the default.
        // Isolates only the Bastion rung: a locally outmatched city halts growth and wants hammers.
        "advanced_city_strategy_bastion_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_comparative = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only the Breadbasket rung: the best food city feeds settlers while the empire is short.
        "advanced_city_strategy_breadbasket_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_comparative = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only the comparative rungs, Forge and Specialist.
        "advanced_city_strategy_comparative_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_pressure = false;
            Box::new(ai)
        }
        // Isolates only per-city military pressure, with no role ever assigned.
        "advanced_city_strategy_pressure_only" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_bastion = false;
            ai.city_strategy_breadbasket = false;
            ai.city_strategy_comparative = false;
            Box::new(ai)
        }
        "advanced_city_strategy_roles_raw" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_emphasis = false;
            ai.city_strategy_expansion_first = false;
            Box::new(ai)
        }
        "advanced_city_strategy_raw" => {
            let mut ai = AdvancedAi::new();
            ai.city_strategy = true;
            ai.city_strategy_halt_growth = true;
            ai.city_strategy_expansion_first = false;
            Box::new(ai)
        }
        // Treatment for the expansion-window axis: identical to `advanced`
        // except the settler gate closes on whether a settler built here and
        // now would pay for itself, rather than on a flat end-of-game reserve.
        // See `AdvancedAi::expansion_pays_back` and #554/#559.
        "advanced_expansion_payback" => {
            let mut ai = AdvancedAi::new();
            ai.expansion_pays_back = true;
            Box::new(ai)
        }
        // The two frozen adaptive-expansion axes and their factorial
        // composition. They are evaluator-only: `advanced` itself leaves both
        // flags false, while the mechanism census proves actual production
        // actions before any outcome seed is read.
        "advanced_late_expansion" => {
            let mut ai = AdvancedAi::new();
            ai.late_expansion = true;
            Box::new(ai)
        }
        "advanced_expansion_dispatch" => {
            let mut ai = AdvancedAi::new();
            ai.expansion_dispatch = true;
            Box::new(ai)
        }
        "advanced_expansion_complete" => {
            let mut ai = AdvancedAi::new();
            ai.late_expansion = true;
            ai.expansion_dispatch = true;
            Box::new(ai)
        }
        // Treatment for the city-target axis: identical to `advanced` except
        // that the target ramp starts at six rather than three. See
        // `AdvancedAi::city_target_floor`, #554 and #569.
        "advanced_wide_opening" => {
            let mut ai = AdvancedAi::new();
            ai.city_target_floor = 6;
            Box::new(ai)
        }
        "advanced_lane_reachable" => {
            let mut ai = AdvancedAi::new();
            ai.refuse_unreachable_lanes = true;
            Box::new(ai)
        }
        // Treatment for the routing axis: identical to `advanced` except that
        // the finite Prophet race is tested before the opportunistic war that
        // currently preempts it on turns 55..120. See
        // `AdvancedAi::prophet_before_opportunism`.
        // Upper bound for the expansion-valuation axis: a settler outbids every
        // other item whenever the five gates permit one at all. This is not a
        // shippable policy, it is the oracle-ablation question — is there ANY
        // headroom in what a settler is worth? The gates still bind (one in
        // flight, stop at the planned target, pop 2, window open), so it means
        // "beeline to the city target", not "settlers forever".
        "advanced_settler_first" => {
            let mut ai = AdvancedAi::new();
            ai.settler_price = 100.0;
            Box::new(ai)
        }
        // A high-rated league genome against the genome the repository evolved.
        // The exhibition now samples each civ's top three eligible entries, so
        // this is a reproducible diagnostic from the shipped roster rather than
        // a claim that every live seat uses this genome. Whether a 1790-rated
        // league genome is genuinely stronger than the champion or merely
        // better-rated is the question this entrant exists to answer;
        // `docs/LEAGUE_GENOME_CHALLENGER.md` records the best-rated one losing
        // 98 Elo when transferred into `strategic_deep`.
        //
        // Picks the highest-rated active genome from the SHIPPED roster, so it
        // is reproducible from the tree rather than from a local league.
        // The cost-performance frontier of the macro search, which nothing has
        // measured because nothing was ever cost-bound. Every registered
        // variant moves the budget UP (`review_every` 20 and 10, `horizon` 80,
        // the 4x `strategic_deep`); this is the first that moves it down.
        //
        // At the measured 6p/74x46 profile, one searching seat among five
        // scripted ones cost 76.7 ms per game-turn versus 13.3 for the
        // all-scripted fleet (6.4x). Search strength across the rotating live
        // profiles remains open, so the deployable question is how much gain
        // survives at a justified budget and profile.
        //
        // Three knobs, all multiplicative on branch-rounds:
        //   review_every 40 -> 80   half the reviews
        //   horizon      40 -> 20   half the rollout
        //   rotate_lanes           ~7 branches -> ~3
        // ≈ 9x cheaper. `rotate_lanes` is a recorded NULL for strength, which
        // is exactly what a cost-bound deployment wants from it.
        "strategic_cheap" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 80;
            ai.horizon = 20;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "advanced_league_top" => {
            let weights = shipped_league_top_advanced().unwrap_or_default();
            Box::new(AdvancedAi::with_weights(weights))
        }
        "advanced_prophet_first" => {
            let mut ai = AdvancedAi::new();
            ai.prophet_before_opportunism = true;
            Box::new(ai)
        }
        // Treatment for the relief-radius axis: identical to `advanced` in
        // every other respect, holding only the force groups that could
        // reach a threatened city instead of every group in the empire.
        // Paired against `advanced` this isolates the scoped hold and
        // nothing else. Measured no stronger at 120 maps, which is why it is
        // an entrant rather than the default; kept so the comparison can be
        // re-run once siege conversion improves.
        "advanced_relief_scoped" => {
            let mut ai = AdvancedAi::new();
            ai.scoped_relief_hold = true;
            Box::new(ai)
        }
        // Treatment for the tactical-commitment axis: identical to `advanced`
        // in every other respect, deciding the turn's whole engagement as one
        // joint problem instead of letting units commit greedily one at a time
        // in a fixed class order. Paired against `advanced` this isolates the
        // commitment rule and nothing else — the same per-unit evaluator, the
        // same weights, the same everything above the battlefield.
        "advanced_joint_tactics" => {
            let mut ai = AdvancedAi::new();
            ai.joint_tactics = true;
            Box::new(ai)
        }
        // The denial ablation on the weights the deployment actually plays.
        // Every other arm in `docs/COUNTERING_LEADERS.md` ran on
        // `Weights::default()`, and a genome moves `war_ratio`, `city_target`
        // and the rest -- so a layer that is worth nothing to the default
        // agent is not automatically worth nothing to the shipped one. Paired
        // against `advanced_evolved`, this is the same ablation on the seat
        // the exhibition fills.
        "advanced_evolved_blind" => {
            let mut ai = crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_else(AdvancedAi::new);
            ai.deny_leaders = false;
            Box::new(ai)
        }
        "advanced_evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_else(AdvancedAi::new),
        ),
        // The Dedication chooser that ranks the offer by what each Dedication
        // would have paid over the era just ended. **A recorded negative
        // result**, kept as an evaluator arm: over 120 mirrored maps against
        // the shipped alphabetical default it took 41.2% of games, 10 map
        // directions to 31, sign p=0.0015, e-process crossing against it at
        // map 51, and terminal score 46.3% (p=0.0000). See `docs/AGES.md`.
        "advanced_measured_dedication" => {
            let mut w = crate::evolve::load_champion("evolved").unwrap_or_default();
            w.dedication_choice = crate::ai::DedicationChoice::Measured;
            Box::new(AdvancedAi::with_weights(w))
        }
        // The repair for that loss: rank on the projection only in a Normal or
        // Dark Age, where Era Score is the literal objective, and leave the
        // Golden and Heroic choice exactly as the default makes it.
        "advanced_banking_dedication" => {
            let mut w = crate::evolve::load_champion("evolved").unwrap_or_default();
            w.dedication_choice = crate::ai::DedicationChoice::Banking;
            Box::new(AdvancedAi::with_weights(w))
        }
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "random" => Box::new(RandomAi::new(seed)),
        // Exact netless fallback played by `neural` when the committed
        // champion is present. Naming it makes provenance collapse checks
        // compare controller *and* weights instead of dropping the genome.
        "basic_evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(BasicAi::with_weights)
                .unwrap_or_default(),
        ),
        "evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_default(),
        ),
        "neural" => {
            let w = crate::evolve::load_champion("evolved").unwrap_or_default();
            match crate::valuenet::ValueNet::load_width("evolved", crate::evolve::FEATURE_WIDTH) {
                Some(n) => Box::new(crate::neural::NeuralAi::new(w, n)),
                None => Box::new(BasicAi::with_weights(w)),
            }
        }
        // `policy` scored with the 34-wide `decision_features` and a net
        // trained on it. The 25-wide vector is unchanged by 96% of the
        // candidates this agent clones; the wide one moves for 69% of unit
        // moves, so this is the first configuration where the tactical
        // evaluator can distinguish the actions it is ranking at all.
        // `policy_wide` denied the one correlate it was measured to be
        // exploiting. A causal test of the ranking failure, not a proposed
        // agent.
        "policy_wide_frozen" => Box::new(
            crate::policy::PolicyAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            )
            .with_frozen_contact(),
        ),
        "policy_wide" => Box::new(
            crate::policy::PolicyAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            )
            .with_decision_features(),
        ),
        "policy" => Box::new(crate::policy::PolicyAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        "strategic" => Box::new(crate::strategic::StrategicAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        // Treatment for the assigned-Religion expansion bypass: identical to
        // `strategic` except that a seat committed to Religion asks the same
        // "can this lane afford to expand first?" question every other assigned
        // lane asks — on the acting agent and on every projected branch alike.
        // See `StrategicAi::set_religion_may_expand`.
        "strategic_religion_expand" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.set_religion_may_expand(true);
            Box::new(ai)
        }
        "strategic_score" => Box::new(crate::strategic::StrategicAi::score_only_with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        // Public-state opponent model. The searching seat, branch set and
        // compute budget are identical to `strategic`; only confidently
        // inferred rival lanes remain fixed through a projection instead of
        // being reconstructed as blank adaptive planners.
        "strategic_rivals" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.model_rival_lanes = true;
            Box::new(ai)
        }
        // Treatment for the doctrine axis: identical to `strategic` in
        // weights, horizon, lane policy and priors, differing only in that
        // a review which reaches the rollouts also projects the four play
        // styles. Paired against `strategic` this isolates the second
        // search axis and nothing else.
        // Search-cadence doses. Everything else — weights, horizon, lane
        // policy, priors — matches `strategic`, so a pair isolates how
        // often the search runs and nothing about how well it runs.
        "strategic_r20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            Box::new(ai)
        }
        "strategic_r10" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            Box::new(ai)
        }
        // Compute-matched cadence: twice the reviews at half the horizon,
        // so total simulated rounds per game match `strategic`. Paired
        // against it, this separates "more decisions" from "more compute".
        "strategic_r20h20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 20;
            Box::new(ai)
        }
        // The other way to spend the doubling `strategic_r20` spends on
        // frequency: same decisions, twice the lookahead. Run on the same
        // maps, the pair asks where a marginal unit of search compute is
        // worth more.
        "strategic_h80" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.horizon = 80;
            Box::new(ai)
        }
        // The macro search with four times the compute, split across both
        // of its axes: reviews every 20 turns instead of 40, projected 80
        // rounds instead of 40. The strongest configuration measured on the
        // 4p, 24x16, Standard source benchmark — promoted on a pre-registered
        // 300-map run at a fresh seed:
        // 56 mirrored maps to 17, sign p=0.0000, e-process 3.14e4 crossing
        // at map 127, Wilson 50.8%..62.0% clearing parity — `promotion
        // gate: PASS` under the unmodified gate. With the two earlier
        // disjoint sets that is 540 independent maps, 109 to 32.
        //
        // `strategic` is deliberately unchanged: it is the frozen control
        // for further search work, the way `advanced_v1` is for
        // `advanced`, and this costs four times the macro-search compute,
        // which batch callers should adopt on purpose rather than inherit.
        "strategic_deep" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // The first strength-first budget above `strategic_deep`: preserve
        // its full 80-round horizon and spend another doubling on the
        // generation-14-favored review cadence. This is deliberately an
        // evaluator-only 8x entrant until an independent promotion gate says
        // that the extra compute buys strength.
        "strategic_ultra" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Frozen control for testing whether the committed AdvancedAi
        // champion transfers through StrategicAi's 20x80 macro search. It
        // retains the same optional value-net path but deliberately refuses
        // best.json, so the genome is the only policy difference. The first
        // transfer screen favored the champion 33-27 games and 5-2 map
        // directions; retained evaluator-only for future artifact audits.
        "strategic_deep_default" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::ai::Weights::default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Same promoted 20x80 search budget, but retain the time-to-terminal
        // signal when several deep branches all win or all lose. Outcome
        // classes remain lexicographic, so this cannot prefer an unresolved
        // score proxy over a projected win or prefer a projected loss over an
        // unresolved branch. Measured 28-32 games on 30 fresh mirrored maps;
        // retained evaluator-only because it did not earn a disjoint gate.
        "strategic_deep_tempo" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.terminal_tempo = true;
            Box::new(ai)
        }
        // Exact one- and two-action search over religious conversions before
        // the ordinary controller takes the rest of the turn. Same promoted
        // 20x80 macro budget as its control. Retained evaluator-only after it
        // lost the disjoint gate 114-126 games and religious wins fell 81-65.
        "strategic_deep_conversion" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.religious_finish_search = true;
            Box::new(ai)
        }
        // Outcome-only repair to the conversion treatment above. It searches
        // the same one- and two-action religious space but acts only when the
        // cloned result is an actual religious victory for this civilization.
        // Retained evaluator-only after two exact 30-30 screens -- fallback
        // and evolved genomes -- with all 60 map directions neutral and
        // identical victory types within each pair.
        "strategic_deep_checkmate" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.religious_checkmate_search = true;
            Box::new(ai)
        }
        // Static genome challengers for the strongest search budget measured
        // on that source benchmark. Unlike `strategic_doctrine`, these do not
        // per-review rollout to choose a play style. Each applies one bounded
        // Doctrine perturbation for the whole game, so a paired evaluation
        // measures whether that policy itself is stronger.
        "strategic_deep_expand" => {
            let weights = crate::strategic::Doctrine::Expand
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        "strategic_deep_consolidate" => {
            let weights = crate::strategic::Doctrine::Consolidate
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        "strategic_deep_militarize" => {
            let weights = crate::strategic::Doctrine::Militarize
                .apply(&crate::evolve::load_champion("evolved").unwrap_or_default());
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // Transfer test for the policy-level evolutionary system: the league
        // rates genomes on completed multiplayer outcomes. Apply its
        // conservatively strongest settled generalist to the promoted search
        // budget, falling back honestly when the committed snapshot is absent.
        "strategic_deep_league" => {
            let weights = league_generalist()
                .map(|(_, weights)| weights)
                .unwrap_or_default();
            let mut ai = crate::strategic::StrategicAi::with_weights(weights);
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        // The same opponent-model treatment on the deepest promoted macro
        // search, isolating whether better branch fidelity still helps when
        // each review already spends the promoted 20x80 budget.
        "strategic_deep_rivals" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.model_rival_lanes = true;
            Box::new(ai)
        }
        // Rollout search over what a city builds, rate-limited to one
        // decision every fifteen turns so the whole feature costs about
        // what the lane search costs.
        // The same search judged by the trained value net rather than
        // score share. It measured identically to `production` (109/240
        // against 108/240), which is the evidence that a net over the same
        // 25 features is a re-weighting of score share and not a second
        // opinion. Kept so the comparison can be re-run.
        "production_net" => Box::new(
            crate::production::ProductionSearchAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            )
            .with_value_net(),
        ),
        "production" => Box::new(crate::production::ProductionSearchAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        // The frozen pre-promotion control: branches projected from a newly
        // constructed planner, which is what every `strategic` number
        // published before 2026-07-26 was measured on. Kept so those numbers
        // stay reproducible now that the promoted behaviour is the default.
        "strategic_cold" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.continue_from_plan = false;
            Box::new(ai)
        }
        // Retained as an explicit name for the promoted behaviour, which is
        // now what `strategic` already does. Kept so the pre-registered runs
        // that earned the promotion can be re-run by name.
        "strategic_warm" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.continue_from_plan = true;
            Box::new(ai)
        }
        // The promoted deep budget, spent adaptively: project every branch
        // in lockstep and stop at the first chunk where they separate,
        // rather than always running the full count. Measured WORSE than
        // its control over 120 mirrored maps -- 39.2% paired score,
        // Elo-equivalent -76, sign p=0.0000, gate RETAIN strategic_deep --
        // and kept as an entrant only so the result stays reproducible.
        //
        // There is deliberately no `strategic_adaptive` at the default
        // horizon of 40. The branches there separate by a median 0.0045,
        // under the 0.01 commitment margin, so the search never stops
        // early and the entrant would be bit-identical to its control —
        // an evaluation of it would measure nothing, which is what #380
        // cost a 240-game run to discover.
        "strategic_deep_adaptive" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.horizon = 80;
            ai.adaptive_horizon = true;
            Box::new(ai)
        }
        // The irreversible-Prophet prior removed, so the rollouts answer the
        // reviews it was short-circuiting. It answers about half of all
        // reviews and the search disagrees with it 85% of the time
        // (`search_probe --priors`), which makes it the largest single
        // restriction on this search that has ever been measured -- and an
        // entirely untested one.
        "strategic_noprophet" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.trust_religious_prior = false;
            Box::new(ai)
        }
        "strategic_rot20" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 20;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "strategic_rot10" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.review_every = 10;
            ai.rotate_lanes = true;
            Box::new(ai)
        }
        "strategic_nodefer" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.defer_periodic_on_interrupt = false;
            Box::new(ai)
        }
        "strategic_doctrine" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.doctrine_search = true;
            Box::new(ai)
        }
        "strategic_joint" => {
            let mut ai = crate::strategic::StrategicAi::with_weights(
                crate::evolve::load_champion("evolved").unwrap_or_default(),
            );
            ai.joint_axis_search = true;
            Box::new(ai)
        }
        "basic" => Box::new(BasicAi::new()),
        _ => unreachable!("registered arm {} has no factory row", kind.name()),
    }
}

/// Build a selectable arm using the production artifact tier, deliberately
/// allowing a missing trained artifact to resolve to its scripted fallback.
///
/// Game-start callers that need to continue without an artifact should use
/// this named escape hatch. Evaluators use [`builtin_ai_strict`] instead, so a
/// result cannot silently be recorded under an unavailable learned name.
pub fn builtin_ai_degraded(name: &str, seed: u64) -> Box<dyn Ai> {
    let requested = ArmKind::from_name(name).unwrap_or(ArmKind::Basic);
    build_arm(artifact_effective_alias(requested, ARTIFACT_DIR), seed)
}

/// Historical compatibility alias for [`builtin_ai_degraded`].
///
/// New callers should make fallback behavior visible by naming
/// `builtin_ai_degraded`; strict evaluator callers use [`builtin_ai_strict`].
pub fn builtin_ai(name: &str, seed: u64) -> Box<dyn Ai> {
    builtin_ai_degraded(name, seed)
}

/// Directory `builtin_ai` resolves trained artifacts from.
pub const ARTIFACT_DIR: &str = "evolved";
/// Evolved strategy genome written by `civvis evolve`.
pub const CHAMPION_FILE: &str = "best.json";
/// Distilled scalar value net written by `tools/train_valuenet.py`.
pub const VALUENET_FILE: &str = "valuenet.json";

/// One trained artifact a builtin name reads, and whether it loaded.
///
/// `definitional` separates the two ways a name depends on an artifact. A
/// definitional artifact *is* the agent: without it `builtin_ai` returns a
/// different agent under the same name. A non-definitional one only tunes
/// the agent, so its absence leaves the name honest but the numbers
/// untrained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStatus {
    pub file: &'static str,
    pub found: bool,
    pub definitional: bool,
}

/// Controller family at the evaluator boundary.  This records the agent that
/// actually receives turns after artifact aliases have resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Advanced,
    LegacyAdvanced,
    Basic,
    Random,
    Neural,
    Policy,
    Strategic,
    Production,
}

/// Origin of the policy weights actually supplied to an arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightSource {
    Stock,
    Champion,
    League,
    FrozenStock,
    Doctrine(&'static str),
}

/// Terminal evaluator or selection rule that the controller actually uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatorSource {
    Scripted,
    Random,
    ScoreShare,
    ValueNet { width: usize },
}

/// What an entrant is on every behavior-defining axis the paired evaluator
/// currently understands.  `canonical` is the typed factory target after
/// artifact resolution, so aliases share a spec by construction rather than
/// by a separately maintained display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub canonical: &'static str,
    pub architecture: Architecture,
    pub weights: WeightSource,
    pub evaluator: EvaluatorSource,
    pub treatments: &'static [&'static str],
}

impl AgentSpec {
    /// The independent axes an evaluator would change by replacing `self`
    /// with `other`.  Treatment components are compared as a set so a
    /// composite reports each mechanism rather than hiding several changes
    /// behind one experiment name.
    pub fn differing_axes(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.architecture != other.architecture {
            out.push("architecture");
        }
        if self.weights != other.weights {
            out.push("weights");
        }
        if self.evaluator != other.evaluator {
            out.push("evaluator");
        }
        for treatment in self
            .treatments
            .iter()
            .chain(other.treatments.iter())
        {
            if self.treatments.contains(treatment) != other.treatments.contains(treatment)
                && !out.contains(treatment)
            {
                out.push(treatment);
            }
        }
        // A newly added factory configuration cannot quietly be called a
        // controlled comparison merely because its semantic tags were not
        // filled in.  The canonical target is part of equality, and remains a
        // conservative final axis until an explicit treatment tag is added.
        if out.is_empty() && self.canonical != other.canonical {
            out.push("implementation");
        }
        out
    }
}

impl ArmKind {
    fn architecture(self) -> Architecture {
        match self {
            Self::Basic | Self::BasicEvolved => Architecture::Basic,
            Self::Random => Architecture::Random,
            Self::Neural => Architecture::Neural,
            Self::Policy | Self::PolicyWide | Self::PolicyWideFrozen => Architecture::Policy,
            Self::Production | Self::ProductionNet => Architecture::Production,
            Self::AdvancedV1 => Architecture::LegacyAdvanced,
            Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConsolidate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepDefault
            | Self::StrategicDeepExpand
            | Self::StrategicDeepLeague
            | Self::StrategicDeepMilitarize
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicReligionExpand
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicScore
            | Self::StrategicUltra
            | Self::StrategicWarm => Architecture::Strategic,
            _ => Architecture::Advanced,
        }
    }

    fn weights(self, champion: bool, league: bool) -> WeightSource {
        match self {
            Self::StrategicDeepDefault => WeightSource::FrozenStock,
            Self::StrategicDeepLeague | Self::AdvancedLeagueTop if league => WeightSource::League,
            Self::StrategicDeepLeague | Self::AdvancedLeagueTop => WeightSource::Stock,
            Self::StrategicDeepExpand => WeightSource::Doctrine("expand"),
            Self::StrategicDeepConsolidate => WeightSource::Doctrine("consolidate"),
            Self::StrategicDeepMilitarize => WeightSource::Doctrine("militarize"),
            Self::AdvancedEvolved | Self::BasicEvolved => WeightSource::Champion,
            Self::Evolved => WeightSource::Champion,
            Self::Neural
            | Self::Policy
            | Self::PolicyWide
            | Self::PolicyWideFrozen
            | Self::Production
            | Self::ProductionNet
            | Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicReligionExpand
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicScore
            | Self::StrategicUltra
            | Self::StrategicWarm
            | Self::AdvancedMeasuredDedication
            | Self::AdvancedEvolvedCommitment
            | Self::AdvancedEvolvedBlind => {
                if champion {
                    WeightSource::Champion
                } else {
                    WeightSource::Stock
                }
            }
            _ => WeightSource::Stock,
        }
    }

    fn evaluator(self, net: bool, wide_net: bool) -> EvaluatorSource {
        match self {
            Self::Random => EvaluatorSource::Random,
            Self::Neural | Self::Policy | Self::ProductionNet if net => {
                EvaluatorSource::ValueNet { width: crate::evolve::FEATURE_WIDTH }
            }
            Self::PolicyWide | Self::PolicyWideFrozen if wide_net => {
                EvaluatorSource::ValueNet { width: crate::decision_features::WIDTH }
            }
            Self::StrategicScore | Self::Production => EvaluatorSource::ScoreShare,
            Self::Strategic
            | Self::StrategicCheap
            | Self::StrategicCold
            | Self::StrategicDeep
            | Self::StrategicDeepAdaptive
            | Self::StrategicDeepCheckmate
            | Self::StrategicDeepConsolidate
            | Self::StrategicDeepConversion
            | Self::StrategicDeepDefault
            | Self::StrategicDeepExpand
            | Self::StrategicDeepLeague
            | Self::StrategicDeepMilitarize
            | Self::StrategicDeepRivals
            | Self::StrategicDeepTempo
            | Self::StrategicDoctrine
            | Self::StrategicH80
            | Self::StrategicJoint
            | Self::StrategicNodefer
            | Self::StrategicNoprophet
            | Self::StrategicR10
            | Self::StrategicR20
            | Self::StrategicR20H20
            | Self::StrategicReligionExpand
            | Self::StrategicRivals
            | Self::StrategicRot10
            | Self::StrategicRot20
            | Self::StrategicUltra
            | Self::StrategicWarm => {
                if net {
                    EvaluatorSource::ValueNet { width: crate::evolve::FEATURE_WIDTH }
                } else {
                    EvaluatorSource::ScoreShare
                }
            }
            _ => EvaluatorSource::Scripted,
        }
    }

    fn treatments(self) -> &'static [&'static str] {
        match self {
            Self::AdvancedBeliefPressure => &["belief-pressure"],
            Self::AdvancedPolicyLiveControl => &["policy-deck-live"],
            Self::AdvancedEnvoyPolicy => &["policy-deck-live", "envoy-influence"],
            Self::AdvancedEnvoyInfrastructure => &["envoy-infrastructure"],
            Self::AdvancedEnvoyPriority => &["envoy-infrastructure", "envoy-priority"],
            Self::AdvancedEnvoyEconomy => &[
                "policy-deck-live",
                "envoy-influence",
                "envoy-infrastructure",
            ],
            Self::AdvancedStrategicCommitment | Self::AdvancedEvolvedCommitment => {
                &["strategy-commitment"]
            }
            Self::AdvancedFoodFirst => &["food-first"],
            Self::AdvancedSettlerCommit => &["settler-commitment"],
            Self::AdvancedBlindToLeaders | Self::AdvancedEvolvedBlind => &["leader-denial-off"],
            Self::AdvancedRush => &["early-rush"],
            Self::AdvancedRushConnected => &["early-rush", "connected-rush"],
            Self::AdvancedCounterInLane => &["counter-in-lane"],
            Self::AdvancedCounterStandDown => &["counter-stand-down"],
            Self::AdvancedEarlyScoreAlarm => &["early-score-alarm"],
            Self::AdvancedCongressCounter => &["congress-counter-target"],
            Self::AdvancedCongressVotes => &["congress-counter-votes"],
            Self::AdvancedCongressCounterHard => {
                &["congress-counter-target", "congress-counter-votes"]
            }
            Self::AdvancedEarlyScoreBuild => &["early-score-alarm", "counter-in-lane"],
            Self::AdvancedCivBlind => &["civilization-blind"],
            Self::AdvancedParallelSettlers => &["parallel-settlers"],
            Self::AdvancedCityStrategy => &["city-directives"],
            Self::AdvancedCityStrategyEmphasis => &["city-directives", "city-emphasis-only"],
            Self::AdvancedCityStrategyRoles => &["city-directives", "city-roles-only"],
            Self::AdvancedCityStrategyRolesRaw => &["city-directives", "city-roles-raw"],
            Self::AdvancedCityStrategyRaw => &["city-directives", "city-raw"],
            Self::AdvancedCityStrategyBastionOnly => &["city-directives", "city-bastion-only"],
            Self::AdvancedCityStrategyBreadbasketOnly => {
                &["city-directives", "city-breadbasket-only"]
            }
            Self::AdvancedCityStrategyComparativeOnly => {
                &["city-directives", "city-comparative-only"]
            }
            Self::AdvancedCityStrategyPressureOnly => &["city-directives", "city-pressure-only"],
            Self::AdvancedExpansionPayback => &["expansion-payback"],
            Self::AdvancedLateExpansion => &["late-expansion"],
            Self::AdvancedExpansionDispatch => &["expansion-dispatch"],
            Self::AdvancedExpansionComplete => &["late-expansion", "expansion-dispatch"],
            Self::AdvancedWideOpening => &["city-target-floor"],
            Self::AdvancedLaneReachable => &["lane-reachability"],
            Self::AdvancedSettlerFirst => &["settler-oracle"],
            Self::AdvancedProphetFirst => &["prophet-priority"],
            Self::AdvancedReliefScoped => &["scoped-relief"],
            Self::AdvancedJointTactics => &["joint-tactics"],
            Self::AdvancedMeasuredDedication => &["dedication-measured"],
            Self::StrategicCheap => &["search-cheap"],
            Self::StrategicCold => &["search-cold"],
            Self::StrategicDeep => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepAdaptive => {
                &["search-cadence-20", "search-horizon-80", "search-adaptive-horizon"]
            }
            Self::StrategicDeepCheckmate => {
                &["search-cadence-20", "search-horizon-80", "religious-checkmate-search"]
            }
            Self::StrategicDeepConversion => {
                &["search-cadence-20", "search-horizon-80", "religious-conversion-search"]
            }
            Self::StrategicDeepDefault => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepExpand => {
                &["search-cadence-20", "search-horizon-80", "doctrine-expand"]
            }
            Self::StrategicDeepConsolidate => {
                &["search-cadence-20", "search-horizon-80", "doctrine-consolidate"]
            }
            Self::StrategicDeepMilitarize => {
                &["search-cadence-20", "search-horizon-80", "doctrine-militarize"]
            }
            Self::StrategicDeepLeague => &["search-cadence-20", "search-horizon-80"],
            Self::StrategicDeepRivals => {
                &["search-cadence-20", "search-horizon-80", "rival-lane-model"]
            }
            Self::StrategicDeepTempo => {
                &["search-cadence-20", "search-horizon-80", "terminal-tempo"]
            }
            Self::StrategicDoctrine => &["doctrine-search"],
            Self::StrategicH80 => &["search-horizon-80"],
            Self::StrategicJoint => &["joint-axis-search"],
            Self::StrategicNodefer => &["search-no-defer"],
            Self::StrategicNoprophet => &["religious-prior-off"],
            Self::StrategicR10 => &["search-cadence-10"],
            Self::StrategicR20 => &["search-cadence-20"],
            Self::StrategicR20H20 => &["search-cadence-20", "search-horizon-20"],
            Self::StrategicReligionExpand => &["religion-may-expand"],
            Self::StrategicRivals => &["rival-lane-model"],
            Self::StrategicRot10 => &["search-cadence-10", "rotate-lanes"],
            Self::StrategicRot20 => &["search-cadence-20", "rotate-lanes"],
            Self::StrategicUltra => &["search-cadence-10", "search-horizon-80"],
            Self::AdvancedV1 => &["legacy-advanced"],
            _ => &[],
        }
    }

    fn spec(self, dir: &str) -> AgentSpec {
        let champion = crate::evolve::load_champion(dir).is_some();
        let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH)
            .is_some();
        let wide_net = crate::valuenet::ValueNet::load_width(
            dir,
            crate::decision_features::WIDTH,
        )
        .is_some();
        let league = match self {
            Self::StrategicDeepLeague => league_generalist().is_some(),
            Self::AdvancedLeagueTop => shipped_league_top_advanced().is_some(),
            _ => false,
        };
        AgentSpec {
            canonical: self.name(),
            architecture: self.architecture(),
            weights: self.weights(champion, league),
            evaluator: self.evaluator(net, wide_net),
            treatments: self.treatments(),
        }
    }
}

/// What a builtin name actually plays as once its artifacts are resolved.
///
/// `builtin_ai` falls back silently when a trained artifact is missing —
/// correctly, because a missing file should not stop a game. What it must
/// not do is let an evaluation record the result under the learned name: on
/// a checkout with no value net, `neural` is either `basic_evolved` or
/// `basic`, and `policy` is either `advanced_evolved` or `advanced`, depending
/// on whether the champion genome loaded. Dropping that second artifact from
/// the effective name makes the self-comparison guard wrong in both
/// directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProvenance {
    /// The name the caller asked for.
    pub requested: String,
    /// Every artifact the name reads, in the order it reads them.
    pub artifacts: Vec<ArtifactStatus>,
    /// Canonical identity of the agent that actually plays. Equals
    /// `requested` unless a definitional artifact is missing or the requested
    /// name is a historical alias of an identical controller and weight set.
    pub effective: &'static str,
}

impl AgentProvenance {
    /// True when the name promises more than the loaded artifacts deliver.
    pub fn degraded(&self) -> bool {
        self.artifacts
            .iter()
            .any(|artifact| artifact.definitional && !artifact.found)
    }

    /// True when some artifact the name reads did not load, whether or not
    /// that changed which agent plays.
    pub fn untrained(&self) -> bool {
        self.artifacts.iter().any(|artifact| !artifact.found)
    }

    pub fn missing(&self) -> Vec<&'static str> {
        self.artifacts
            .iter()
            .filter(|artifact| !artifact.found)
            .map(|artifact| artifact.file)
            .collect()
    }

    /// One reportable line, e.g.
    /// `neural: plays as basic (missing valuenet.json, best.json)`.
    pub fn line(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            return if self.artifacts.is_empty() {
                format!("{}: scripted, no artifacts required", self.requested)
            } else if self.effective != self.requested {
                format!(
                    "{}: plays as {} (loaded {})",
                    self.requested,
                    self.effective,
                    self.artifacts_list()
                )
            } else {
                format!("{}: loaded {}", self.requested, self.artifacts_list())
            };
        }
        let plays = match self.degraded() {
            true => format!("plays as {}", self.effective),
            false => format!("plays as {} with untrained defaults", self.requested),
        };
        format!("{}: {} (missing {})", self.requested, plays, missing.join(", "))
    }

    fn artifacts_list(&self) -> String {
        self.artifacts
            .iter()
            .map(|artifact| artifact.file)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Why strict builtin construction refused a requested evaluator arm.
///
/// An artifact can be absent without changing the controller (for example, an
/// optional tuning net). Only [`Self::Degraded`] rejects construction: it
/// means a definitional artifact is absent and the requested name would play
/// as a different controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinAiBuildError {
    /// No selectable builtin or evaluator-only arm has this name.
    UnknownName { requested: String },
    /// A definitional artifact did not load, so construction would substitute
    /// the effective scripted fallback recorded in this provenance.
    Degraded { provenance: AgentProvenance },
}

impl BuiltinAiBuildError {
    /// The name the caller attempted to construct.
    pub fn requested(&self) -> &str {
        match self {
            Self::UnknownName { requested } => requested,
            Self::Degraded { provenance } => &provenance.requested,
        }
    }

    /// Detailed artifact resolution when a known arm degraded.
    pub fn provenance(&self) -> Option<&AgentProvenance> {
        match self {
            Self::UnknownName { .. } => None,
            Self::Degraded { provenance } => Some(provenance),
        }
    }
}

impl std::fmt::Display for BuiltinAiBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownName { requested } => write!(f, "unknown builtin AI {requested:?}"),
            Self::Degraded { provenance } => write!(
                f,
                "{} is unavailable and would play as {} (missing {})",
                provenance.requested,
                provenance.effective,
                provenance.missing().join(", ")
            ),
        }
    }
}

impl std::error::Error for BuiltinAiBuildError {}

/// A resolved evaluator arm. The private `kind` is the exact canonical target
/// sent to the factory; public consumers can compare `spec` without relying on
/// a display alias or a second factory match.
#[derive(Debug, Clone)]
pub struct BuiltinArm {
    pub spec: AgentSpec,
    kind: ArmKind,
}

impl BuiltinArm {
    pub fn build(&self, seed: u64) -> Box<dyn Ai> {
        build_arm(self.kind, seed)
    }
}

fn builtin_arm_in(name: &str, dir: &str) -> Option<BuiltinArm> {
    let requested = ArmKind::from_name(name)?;
    let effective = artifact_effective_alias(requested, dir);
    Some(BuiltinArm {
        spec: effective.spec(dir),
        kind: effective,
    })
}

/// Resolve one selectable arm at the production artifact tier. Returning
/// `None` for an unknown name makes strict evaluator callers fail closed while
/// [`builtin_ai`] retains the explicit game-start fallback for legacy callers.
pub fn builtin_arm(name: &str) -> Option<BuiltinArm> {
    builtin_arm_in(name, ARTIFACT_DIR)
}

/// Resolve a known arm only when its definitional artifacts loaded.
///
/// Keeping the resolver separate makes the invariant testable against a
/// fixture directory; public construction below always uses the production
/// artifact tier that the factories actually read.
fn strict_builtin_arm_in(name: &str, dir: &str) -> Result<BuiltinArm, BuiltinAiBuildError> {
    let arm = builtin_arm_in(name, dir).ok_or_else(|| BuiltinAiBuildError::UnknownName {
        requested: name.to_string(),
    })?;
    let provenance = builtin_provenance(name, dir);
    debug_assert_eq!(
        arm.spec.canonical, provenance.effective,
        "strict arm and provenance disagree for {name}"
    );
    if provenance.degraded() {
        return Err(BuiltinAiBuildError::Degraded { provenance });
    }
    Ok(arm)
}

/// Build a selectable arm only when the requested identity is available.
///
/// This is the fail-closed evaluator boundary. It rejects unknown names and
/// names whose missing definitional artifact would silently substitute a
/// different agent. Call [`builtin_ai_degraded`] only when continuing with
/// that substitution is an intentional, reportable choice.
pub fn builtin_ai_strict(name: &str, seed: u64) -> Result<Box<dyn Ai>, BuiltinAiBuildError> {
    Ok(strict_builtin_arm_in(name, ARTIFACT_DIR)?.build(seed))
}

/// Resolve what `builtin_ai(name, _)` will actually construct from `dir`.
///
/// Presence is decided by the same loaders the agents use, not by a stat:
/// a `valuenet.json` that fails `ValueNet::valid` is rejected at load time,
/// so reporting it as present would restate the bug it is meant to catch.
pub fn builtin_provenance(name: &str, dir: &str) -> AgentProvenance {
    let kind = ArmKind::from_name(name).unwrap_or(ArmKind::Basic);
    let champion = crate::evolve::load_champion(dir).is_some();
    let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let wide_net =
        crate::valuenet::ValueNet::load_width(dir, crate::decision_features::WIDTH).is_some();
    let basic_fallback = if champion { "basic_evolved" } else { "basic" };
    let advanced_fallback = if champion {
        "advanced_evolved"
    } else {
        "advanced"
    };
    let genome = ArtifactStatus {
        file: CHAMPION_FILE,
        found: champion,
        definitional: false,
    };
    let value = |definitional| ArtifactStatus {
        file: VALUENET_FILE,
        found: net,
        definitional,
    };
    let league = league_generalist().is_some();
    let (artifacts, independently_declared_effective) = match name {
        // The genome *is* these two names; without it they are the stock
        // scripted agent under a name that claims otherwise.
        "evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                // `evolved` and `advanced_evolved` construct the same
                // `AdvancedAi::with_weights(champion)`; retain one canonical
                // identity so their comparison is rejected as self-play.
                "advanced_evolved"
            } else {
                "advanced"
            },
        ),
        "advanced_evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                "advanced_evolved"
            } else {
                "advanced"
            },
        ),
        "basic_evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            basic_fallback,
        ),
        // NeuralAi needs the net to exist at all and drops all the way to
        // the lightweight controller without it. Preserve whether that
        // controller loaded the champion genome in the effective identity.
        "neural" => (
            vec![genome, value(true)],
            if net { "neural" } else { basic_fallback },
        ),
        "policy" => (
            vec![genome, value(true)],
            if net { "policy" } else { advanced_fallback },
        ),
        // The *wide* net is definitional and is a different artifact from
        // the one `policy` wants: `load_width` refuses each to the other,
        // so without a 34-wide net in place this is the scripted agent.
        "policy_wide" | "policy_wide_frozen" => (
            vec![
                genome,
                ArtifactStatus {
                    file: VALUENET_FILE,
                    found: wide_net,
                    definitional: true,
                },
            ],
            if wide_net {
                if name == "policy_wide" {
                    "policy_wide"
                } else {
                    "policy_wide_frozen"
                }
            } else {
                advanced_fallback
            },
        ),
        // Strategic keeps its lane rollouts without a net; what it loses is
        // the learned terminal evaluator, which is exactly the published
        // `strategic_score` control.
        "strategic" => (
            vec![genome, value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        // The control refuses a net by construction, so it is never
        // degraded — only untrained when the genome is absent.
        "strategic_score" => (vec![genome], "strategic_score"),
        "strategic_rivals" => (vec![genome, value(false)], "strategic_rivals"),
        // Unlike `strategic`, its netless form has no separate published
        // name to degrade *to*: the doctrine axis runs either way. A
        // missing net therefore leaves it untrained rather than renamed,
        // which the provenance line says in those words.
        "strategic_doctrine" => (vec![genome, value(false)], "strategic_doctrine"),
        "strategic_joint" => (vec![genome, value(false)], "strategic_joint"),
        "strategic_r20" => (vec![genome, value(false)], "strategic_r20"),
        "strategic_r10" => (vec![genome, value(false)], "strategic_r10"),
        "strategic_nodefer" => (vec![genome, value(false)], "strategic_nodefer"),
        "strategic_r20h20" => (vec![genome, value(false)], "strategic_r20h20"),
        "strategic_h80" => (vec![genome, value(false)], "strategic_h80"),
        "strategic_rot20" => (vec![genome, value(false)], "strategic_rot20"),
        "strategic_warm" => (
            vec![genome, value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        "strategic_religion_expand" => (
            vec![genome, value(false)],
            "strategic_religion_expand",
        ),
        "strategic_cold" => (vec![genome, value(false)], "strategic_cold"),
        "strategic_noprophet" => (vec![genome, value(false)], "strategic_noprophet"),
        "strategic_deep_adaptive" => (vec![genome, value(false)], "strategic_deep_adaptive"),
        // Same artifact dependencies as `strategic`: the genome tunes it,
        // and the net is non-definitional because the search runs without
        // one. There is no separate published netless name to degrade to.
        "strategic_deep" => (vec![genome, value(false)], "strategic_deep"),
        "strategic_ultra" => (vec![genome, value(false)], "strategic_ultra"),
        // The frozen genome is in code; only the same optional value net read
        // by `strategic_deep` remains in its provenance.
        "strategic_deep_default" => (vec![value(false)], "strategic_deep_default"),
        "strategic_deep_tempo" => (
            vec![genome, value(false)],
            "strategic_deep_tempo",
        ),
        "strategic_deep_conversion" => (
            vec![genome, value(false)],
            "strategic_deep_conversion",
        ),
        "strategic_deep_checkmate" => (
            vec![genome, value(false)],
            "strategic_deep_checkmate",
        ),
        "strategic_deep_expand" => (vec![genome, value(false)], "strategic_deep_expand"),
        "strategic_deep_consolidate" => (vec![genome, value(false)], "strategic_deep_consolidate"),
        "strategic_deep_militarize" => (vec![genome, value(false)], "strategic_deep_militarize"),
        "strategic_deep_league" => (
            vec![ArtifactStatus {
                file: LEAGUE_SNAPSHOT_FILE,
                found: league,
                definitional: true,
            }],
            if league {
                "strategic_deep_league"
            } else {
                "strategic_deep"
            },
        ),
        "strategic_deep_rivals" => (vec![genome, value(false)], "strategic_deep_rivals"),
        "strategic_rot10" => (vec![genome, value(false)], "strategic_rot10"),
        // The genome tunes both its rollout policy and its scripted
        // governor; it consults no net.
        "production" => (vec![genome], "production"),
        // The net is definitional: without it this is exactly `production`.
        "production_net" => (
            vec![genome, value(true)],
            if net { "production_net" } else { "production" },
        ),
        "advanced" => (Vec::new(), "advanced"),
        "advanced_belief_pressure" => (Vec::new(), "advanced_belief_pressure"),
        "advanced_policy_live_control" => (Vec::new(), "advanced_policy_live_control"),
        "advanced_envoy_policy" => (Vec::new(), "advanced_envoy_policy"),
        "advanced_envoy_infrastructure" => (Vec::new(), "advanced_envoy_infrastructure"),
        "advanced_envoy_priority" => (Vec::new(), "advanced_envoy_priority"),
        "advanced_envoy_economy" => (Vec::new(), "advanced_envoy_economy"),
        "advanced_strategic_commitment" => (Vec::new(), "advanced_strategic_commitment"),
        "advanced_evolved_commitment" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                "advanced_evolved_commitment"
            } else {
                "advanced_strategic_commitment"
            },
        ),
        "advanced_lane_reachable" => (Vec::new(), "advanced_lane_reachable"),
        "advanced_wide_opening" => (Vec::new(), "advanced_wide_opening"),
        "advanced_expansion_payback" => (Vec::new(), "advanced_expansion_payback"),
        "advanced_late_expansion" => (Vec::new(), "advanced_late_expansion"),
        "advanced_expansion_dispatch" => (Vec::new(), "advanced_expansion_dispatch"),
        "advanced_expansion_complete" => (Vec::new(), "advanced_expansion_complete"),
        "advanced_city_strategy" => (Vec::new(), "advanced_city_strategy"),
        "advanced_city_strategy_emphasis" => (Vec::new(), "advanced_city_strategy_emphasis"),
        "advanced_city_strategy_roles" => (Vec::new(), "advanced_city_strategy_roles"),
        "advanced_city_strategy_roles_raw" => (Vec::new(), "advanced_city_strategy_roles_raw"),
        "advanced_city_strategy_raw" => (Vec::new(), "advanced_city_strategy_raw"),
        "advanced_city_strategy_bastion_only" => (Vec::new(), "advanced_city_strategy_bastion_only"),
        "advanced_city_strategy_breadbasket_only" => (Vec::new(), "advanced_city_strategy_breadbasket_only"),
        "advanced_city_strategy_comparative_only" => (Vec::new(), "advanced_city_strategy_comparative_only"),
        "advanced_city_strategy_pressure_only" => (Vec::new(), "advanced_city_strategy_pressure_only"),
        "advanced_banking_dedication" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            advanced_fallback,
        ),
        "advanced_measured_dedication" => (vec![genome], "advanced_measured_dedication"),
        "advanced_parallel_settlers" => (Vec::new(), "advanced_parallel_settlers"),
        "advanced_settler_first" => (Vec::new(), "advanced_settler_first"),
        "advanced_prophet_first" => (Vec::new(), "advanced_prophet_first"),
        "advanced_league_top" => (Vec::new(), "advanced_league_top"),
        "strategic_cheap" => (vec![genome, value(false)], "strategic_cheap"),
        "advanced_blind_to_leaders" => (Vec::new(), "advanced_blind_to_leaders"),
        "advanced_counter_in_lane" => (Vec::new(), "advanced_counter_in_lane"),
        "advanced_congress_counter" => (Vec::new(), "advanced_congress_counter"),
        "advanced_congress_votes" => (Vec::new(), "advanced_congress_votes"),
        "advanced_congress_counter_hard" => (Vec::new(), "advanced_congress_counter_hard"),
        "advanced_counter_stand_down" => (Vec::new(), "advanced_counter_stand_down"),
        "advanced_early_score_alarm" => (Vec::new(), "advanced_early_score_alarm"),
        "advanced_early_score_build" => (Vec::new(), "advanced_early_score_build"),
        // The genome is definitional here for the same reason it is for
        // `advanced_evolved`: without it this is the stock agent with the
        // denial layer off, which is a different measurement entirely.
        "advanced_evolved_blind" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion {
                "advanced_evolved_blind"
            } else {
                "advanced_blind_to_leaders"
            },
        ),
        "advanced_civ_blind" => (Vec::new(), "advanced_civ_blind"),
        "advanced_rush" => (Vec::new(), "advanced_rush"),
        "advanced_rush_connected" => (Vec::new(), "advanced_rush_connected"),
        "advanced_settler_commit" => (Vec::new(), "advanced_settler_commit"),
        "advanced_food_first" => (Vec::new(), "advanced_food_first"),
        "advanced_v1" => (Vec::new(), "advanced_v1"),
        "advanced_relief_scoped" => (Vec::new(), "advanced_relief_scoped"),
        "advanced_joint_tactics" => (Vec::new(), "advanced_joint_tactics"),
        "random" => (Vec::new(), "random"),
        // `builtin_ai` answers every other name with the lightweight agent.
        "basic" => (Vec::new(), "basic"),
        _ => (Vec::new(), "basic"),
    };
    let effective = artifact_effective_alias_from(kind, champion, net, wide_net, league).name();
    debug_assert_eq!(
        effective, independently_declared_effective,
        "artifact alias table and provenance row diverged for {name}"
    );
    AgentProvenance {
        requested: name.to_string(),
        artifacts,
        effective,
    }
}

/// Provenance for a whole entrant list, in the order given.
pub fn builtin_provenances(names: &[&str], dir: &str) -> Vec<AgentProvenance> {
    names
        .iter()
        .map(|name| builtin_provenance(name, dir))
        .collect()
}

/// Distinct requested names that resolve to the same agent, which makes any
/// difference between them noise. Returns `(first, second, shared agent)`.
pub fn collapsed_entrants(names: &[&str], dir: &str) -> Vec<(String, String, &'static str)> {
    let resolved = names
        .iter()
        .filter_map(|name| builtin_arm_in(name, dir).map(|arm| (*name, arm)))
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for (index, left) in resolved.iter().enumerate() {
        for right in resolved.iter().skip(index + 1) {
            if left.0 != right.0 && left.1.spec == right.1.spec {
                out.push((
                    left.0.to_string(),
                    right.0.to_string(),
                    left.1.spec.canonical,
                ));
            }
        }
    }
    out
}

pub struct TourneyCfg {
    pub games: u32,
    pub players_per_game: usize,
    pub width: i32,
    pub height: i32,
    pub speed: String,
    pub map_script: MapScript,
    pub map_topology: MapTopology,
    pub map_poles: MapPoles,
    pub max_turns: u32,
    pub num_city_states: usize,
    pub seed: u64,
    pub k: f64,
    /// Immutable player identity that pins the longitudinal rating scale.
    /// `None` leaves an in-memory or one-off pool floating around its base.
    pub rating_anchor: Option<String>,
    /// Ordered controller roles behind the versioned rating identities.
    /// Persistent CLI tournaments require one role per entrant; in-memory
    /// library experiments may leave this empty.
    pub controller_roster: Vec<String>,
    pub verbose: bool,
    /// How many games to play at once. Results and rating checkpoints remain
    /// in game order, so concurrency does not change the final table.
    pub jobs: usize,
}

impl Default for TourneyCfg {
    fn default() -> Self {
        let size = MapSize::for_players(4);
        let speed = default_speed();
        let max_turns = Rules::embedded()
            .speeds
            .get(&speed)
            .map_or(500, |spec| spec.turns);
        TourneyCfg {
            games: 20,
            players_per_game: 4,
            width: size.width,
            height: size.height,
            speed,
            map_script: MapScript::default(),
            map_topology: MapTopology::default(),
            map_poles: MapPoles::default(),
            max_turns,
            num_city_states: size.default_city_states,
            seed: 0,
            k: 24.0,
            rating_anchor: None,
            controller_roster: Vec::new(),
            verbose: true,
            jobs: crate::parallel::default_jobs(),
        }
    }
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Build a seeded round-robin order. The stride is coprime with the entrant
/// count, so every fixed civilization seat sees every entrant exactly once in
/// each complete cycle. When there are no more entrants than seats, every game
/// also contains every entrant at least once.
fn seat_schedule(names: &[String], players: usize, rng: &mut Rng) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..names.len()).collect();
    for index in (1..order.len()).rev() {
        let other = rng.below(index + 1);
        order.swap(index, other);
    }
    let mut stride = players % names.len();
    if stride == 0 {
        stride = 1;
    }
    while gcd(stride, names.len()) != 1 {
        stride = stride % names.len() + 1;
    }
    (order, stride)
}

fn scheduled_seats(
    names: &[String],
    players: usize,
    game: u32,
    order: &[usize],
    stride: usize,
) -> Vec<String> {
    (0..players)
        .map(|seat| {
            let scheduled = (game as usize * stride + seat) % names.len();
            names[order[scheduled]].clone()
        })
        .collect()
}

fn play_tournament<F, C, E>(
    names: &[String],
    make: &F,
    cfg: &TourneyCfg,
    mut checkpoint: C,
) -> Result<(), E>
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
    C: FnMut(u32, u64, &[RatedPlayer]) -> Result<(), E>,
{
    assert!(!names.is_empty(), "no entrants");
    assert!(cfg.players_per_game >= 2, "Elo needs at least two players");
    let mut rng = Rng::new(cfg.seed.wrapping_add(0x5EED));
    let (entrant_order, entrant_stride) = seat_schedule(names, cfg.players_per_game, &mut rng);
    let draws: Vec<(u64, Vec<String>)> = (0..cfg.games)
        .map(|game| {
            (
                cfg.seed.wrapping_mul(100_000).wrapping_add(game as u64),
                scheduled_seats(
                    names,
                    cfg.players_per_game,
                    game,
                    &entrant_order,
                    entrant_stride,
                ),
            )
        })
        .collect();

    // Games are independent and expensive, while rating mutation and
    // persistence remain serialized below in deterministic game order.
    let played = crate::parallel::map(draws.len(), cfg.jobs, |game_index| {
        let (gseed, seats) = &draws[game_index];
        let mut options = GameOptions::new(
            cfg.players_per_game,
            cfg.width,
            cfg.height,
            *gseed,
            cfg.max_turns,
            cfg.num_city_states,
        );
        options.speed = cfg.speed.clone();
        options.map_script = cfg.map_script;
        options.map_topology = cfg.map_topology;
        options.map_poles = cfg.map_poles;
        let mut game = Game::new_with(options);
        let mut ais: Vec<Box<dyn Ai>> = game
            .players
            .iter()
            .map(|player| {
                if player.id < cfg.players_per_game {
                    make(&seats[player.id], gseed.wrapping_add(player.id as u64))
                } else {
                    builtin_ai("basic", gseed.wrapping_add(player.id as u64))
                }
            })
            .collect();
        while game.winner.is_none() {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }

        // A game nobody won is a game nobody won: every seat is rated as a
        // non-winner, and the ratings fall back to the score ordering they
        // already carry. Only a lobby that switched the score victory off can
        // reach this, but it must not take the rating run down with it.
        let winner = game.winner;
        let results: Vec<RatedPlayer> = (0..cfg.players_per_game)
            .map(|pid| {
                let civilization = game.players[pid].civ.clone();
                let leader = game
                    .rules
                    .civs
                    .get(&civilization)
                    .map(|spec| spec.leader.clone())
                    .unwrap_or_else(|| civilization.clone());
                RatedPlayer::new(
                    seats[pid].clone(),
                    leader,
                    civilization,
                    game.score(pid),
                    winner == Some(pid),
                )
            })
            .collect();
        let wname = match winner {
            Some(winner) if winner < cfg.players_per_game => seats[winner].clone(),
            Some(winner) => game.players[winner].civ.clone(),
            None => "-".to_string(),
        };
        (
            *gseed,
            results,
            wname,
            winner.map_or_else(
                || "-".to_string(),
                |winner| game.players[winner].civ.clone(),
            ),
            game.victory_type.clone().unwrap_or_default(),
            game.reported_turn(),
        )
    });

    for (game_index, (gseed, results, winner, civilization, victory, turn)) in
        played.into_iter().enumerate()
    {
        checkpoint(game_index as u32, gseed, &results)?;
        if cfg.verbose {
            let labels: Vec<String> = results
                .iter()
                .map(|result| {
                    format!(
                        "{}:{}:{}",
                        result.key.player, result.key.leader, result.key.civilization
                    )
                })
                .collect();
            println!(
                "game {game_index:3}  winner={winner:<10} \
                 ({civilization}, {victory}, t{turn})  seats={labels:?}",
            );
        }
    }
    Ok(())
}

pub fn run_tournament<F>(names: &[String], make: F, cfg: &TourneyCfg) -> EloPool
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    let mut pool = EloPool::new(names, ELO_BASE_RATING);
    pool.bind_profile(TournamentProfile::from_cfg(cfg))
        .expect("TourneyCfg always produces a valid rating profile");
    let result: Result<(), std::convert::Infallible> =
        play_tournament(names, &make, cfg, |_, _, players| {
            pool.record_game(players, cfg.k);
            Ok(())
        });
    match result {
        Ok(()) => pool,
        Err(never) => match never {},
    }
}

pub fn run_tournament_into<F>(names: &[String], make: F, cfg: &TourneyCfg, pool: &mut EloPool)
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    pool.bind_profile(TournamentProfile::from_cfg(cfg))
        .expect("cannot mix tournament profiles in one Elo pool");
    let result: Result<(), std::convert::Infallible> =
        play_tournament(names, &make, cfg, |_, _, players| {
            pool.record_game(players, cfg.k);
            Ok(())
        });
    if let Err(never) = result {
        match never {}
    }
}

struct LedgerLock {
    path: PathBuf,
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_ledger_lock(path: &Path) -> io::Result<LedgerLock> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("elo_ratings.json");
    let lock_path = path.with_file_name(format!(".{file_name}.lock"));
    if let Some(parent) = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    for _ in 0..400 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                if let Err(error) = writeln!(file, "{}", std::process::id()) {
                    let _ = fs::remove_file(&lock_path);
                    return Err(error);
                }
                return Ok(LedgerLock { path: lock_path });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::WouldBlock,
        format!(
            "timed out waiting for Elo ledger lock {}",
            lock_path.display()
        ),
    ))
}

#[cfg(test)]
fn update_ledger(path: &Path, update: impl FnOnce(&mut EloPool)) -> io::Result<EloPool> {
    let _lock = acquire_ledger_lock(path)?;
    let mut pool = EloPool::load_or_new(path, ELO_BASE_RATING)?;
    update(&mut pool);
    pool.save(path)?;
    Ok(pool)
}

fn update_profiled_ledger(
    path: &Path,
    profile: &TournamentProfile,
    update: impl FnOnce(&mut EloPool) -> io::Result<()>,
) -> io::Result<EloPool> {
    let _lock = acquire_ledger_lock(path)?;
    let mut pool = EloPool::load_or_new(path, ELO_BASE_RATING)?;
    pool.bind_profile(profile.clone())?;
    update(&mut pool)?;
    pool.save(path)?;
    Ok(pool)
}

fn tournament_event_id(
    run_seed: u64,
    game_index: u32,
    map_seed: u64,
    players: &[RatedPlayer],
) -> String {
    let seats = players
        .iter()
        .map(|player| {
            let name = &player.key.player;
            format!("{}:{name}", name.len())
        })
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "v{ELO_PROTOCOL_VERSION}:{run_seed:020}:{game_index:010}:{map_seed:020}:{seats}"
    )
}

/// Run a tournament against the latest shared ledger and atomically checkpoint
/// every completed game. `cfg.controller_roster` must name the ordered,
/// fixed controller role behind each versioned identity in `names`. The
/// short per-game lock prevents concurrent agents from overwriting one
/// another's updates.
pub fn run_persistent_tournament<F>(
    names: &[String],
    make: F,
    cfg: &TourneyCfg,
    path: impl AsRef<Path>,
) -> io::Result<EloPool>
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    if names.len() < cfg.players_per_game {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "persistent Elo needs at least {} distinct entrants for {} seats; cloned seats change the contest, so add anchors or use an in-memory tournament",
                cfg.players_per_game, cfg.players_per_game,
            ),
        ));
    }
    let distinct: BTreeSet<&String> = names.iter().collect();
    if distinct.len() != names.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "persistent Elo entrant names must be unique",
        ));
    }
    if let Some(anchor) = &cfg.rating_anchor {
        if !distinct.contains(anchor) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("rating anchor {anchor:?} must be one of the tournament entrants"),
            ));
        }
    }
    if cfg.controller_roster.len() != names.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "persistent Elo needs one ordered controller role per entrant ({} identities, {} controllers)",
                names.len(),
                cfg.controller_roster.len(),
            ),
        ));
    }
    let path = path.as_ref();
    let profile = TournamentProfile::from_cfg(cfg);
    let mut pool = update_profiled_ledger(path, &profile, |_| Ok(()))?;
    play_tournament(names, &make, cfg, |game_index, map_seed, players| {
        let event_id = tournament_event_id(cfg.seed, game_index, map_seed, players);
        pool = update_profiled_ledger(path, &profile, |latest| {
            latest
                .record_game_once(event_id, players, cfg.k)
                .map(|_| ())
        })?;
        Ok::<(), io::Error>(())
    })?;
    Ok(pool)
}

pub fn leaderboard(pool: &EloPool) -> String {
    let mut overall: Vec<(&String, &Rating)> = pool.overall.iter().collect();
    overall.sort_by(|(name_a, a), (name_b, b)| {
        b.elo.total_cmp(&a.elo).then(name_a.cmp(name_b))
    });
    let mut out = String::new();
    if let Some(profile) = &pool.profile {
        out.push_str(&format!("rating profile: {}\n", profile.label()));
    } else {
        out.push_str("rating profile: unbound (migrated/manual pool)\n");
    }
    if pool.history_complete {
        out.push_str(&format!(
            "rating evidence: {} raw games (complete and replay-verified)\n",
            pool.history.len()
        ));
    } else {
        out.push_str(&format!(
            "rating evidence: {} raw games after an unreconstructable legacy prior\n",
            pool.history.len()
        ));
    }
    out.push_str(
        "Anchored online Elo leaderboard (order-sensitive K-factor path, player across all draws):\n",
    );
    for (player, rating) in overall {
        out.push_str(&format!(
            "  {:<24} {:7.1}   games={:<4} wins={:<4} winrate={:>3.0}%\n",
            player,
            rating.elo,
            rating.games,
            rating.wins,
            100.0 * rating.wins as f64 / rating.games.max(1) as f64,
        ));
    }
    if let Some(anchor) = pool
        .profile
        .as_ref()
        .and_then(|profile| profile.rating_anchor.as_deref())
    {
        let mut performance = direct_anchor_performance(pool, anchor);
        performance.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        if !performance.is_empty() {
            let evidence = if pool.history_complete {
                "Standardized direct performance"
            } else {
                "Post-migration direct performance (retained raw games only; legacy prior excluded)"
            };
            out.push_str(&format!(
                "\n{evidence} Elo vs {anchor} (order-independent Jeffreys point; 95% Wilson interval transformed to Elo):\n"
            ));
            for (player, elo, score, games, low, high) in performance {
                let elo_low = performance_elo(pool.base_rating, low);
                let elo_high = performance_elo(pool.base_rating, high);
                out.push_str(&format!(
                    "  {:<24} {:7.1} (95% {:7.1}..{:7.1})   pair-score={:>5.1}/{:<4} ({:>4.1}%, 95% {:>4.1}..{:>4.1}%)\n",
                    player,
                    elo,
                    elo_low,
                    elo_high,
                    score,
                    games,
                    100.0 * score / games as f64,
                    100.0 * low,
                    100.0 * high,
                ));
            }
        }
    }
    out.push_str("\nElo by player × leader × civilization:\n");
    let mut rows: Vec<(&RatingKey, &Rating)> = pool.ratings.iter().collect();
    rows.sort_by(|(key_a, a), (key_b, b)| {
        b.elo
            .total_cmp(&a.elo)
            .then(key_a.player.cmp(&key_b.player))
            .then(key_a.leader.cmp(&key_b.leader))
            .then(key_a.civilization.cmp(&key_b.civilization))
    });
    for (key, rating) in rows {
        out.push_str(&format!(
            "  {:<18} {:<18} {:<12} {:7.1}   games={:<4} wins={:<4} winrate={:>3.0}%\n",
            key.player,
            key.leader,
            key.civilization,
            rating.elo,
            rating.games,
            rating.wins,
            100.0 * rating.wins as f64 / rating.games.max(1) as f64,
        ));
    }
    out
}

/// Maximum-likelihood-style performance ratings directly against the fixed
/// control, derived from raw games rather than the order-sensitive K-factor
/// path. A Jeffreys half-result on each side keeps an undefeated or winless
/// finite sample finite without pretending it was observed.
fn wilson_interval(score: f64, games: usize) -> (f64, f64) {
    if games == 0 {
        return (0.0, 1.0);
    }
    let n = games as f64;
    let p = (score / n).clamp(0.0, 1.0);
    let z = 1.96;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denominator;
    let margin = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denominator;
    ((centre - margin).clamp(0.0, 1.0), (centre + margin).clamp(0.0, 1.0))
}

fn performance_elo(base: f64, pair_score: f64) -> f64 {
    let probability = pair_score.clamp(0.0, 1.0);
    base + 400.0 * (probability / (1.0 - probability)).log10()
}

fn direct_anchor_performance(
    pool: &EloPool,
    anchor: &str,
) -> Vec<(String, f64, f64, usize, f64, f64)> {
    let mut evidence = BTreeMap::<String, (f64, usize)>::new();
    for game in &pool.history {
        let anchor_seats: Vec<&RatedPlayer> = game
            .players
            .iter()
            .filter(|seat| seat.key.player == anchor)
            .collect();
        if anchor_seats.is_empty() {
            continue;
        }
        let opponents: BTreeSet<&str> = game
            .players
            .iter()
            .map(|seat| seat.key.player.as_str())
            .filter(|player| *player != anchor)
            .collect();
        for opponent in opponents {
            let opponent_seats: Vec<&RatedPlayer> = game
                .players
                .iter()
                .filter(|seat| seat.key.player == opponent)
                .collect();
            let mut score = 0.0;
            let mut comparisons = 0usize;
            for challenger in &opponent_seats {
                for control in &anchor_seats {
                    score += head_to_head_score(challenger, control);
                    comparisons += 1;
                }
            }
            let result = score / comparisons.max(1) as f64;
            let aggregate = evidence.entry(opponent.to_string()).or_default();
            aggregate.0 += result;
            aggregate.1 += 1;
        }
    }
    evidence
        .into_iter()
        .filter_map(|(player, (score, games))| {
            (games > 0).then(|| {
                let probability = (score + 0.5) / (games as f64 + 1.0);
                let elo = performance_elo(pool.base_rating, probability);
                let (low, high) = wilson_interval(score, games);
                (player, elo, score, games, low, high)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_ai, builtin_ai_degraded, builtin_ai_strict, builtin_arm, builtin_provenance,
        collapsed_entrants, direct_anchor_performance, expected, leaderboard, league_generalist,
        performance_elo, scheduled_seats, seat_schedule, strict_builtin_arm_in, wilson_interval,
        win_shares, BuiltinAiBuildError, EloPool, RatedPlayer, RatingKey, TournamentProfile,
        TourneyCfg, WeightSource, ARTIFACT_DIR, BUILTIN_AIS, CHAMPION_FILE, DEFAULT_RATINGS_PATH,
        ELO_BASE_RATING, ELO_SCHEMA_VERSION, EVAL_ONLY_AIS, HISTORICAL_V1_RATINGS_PATH,
        VALUENET_FILE,
    };
    use crate::game::{Action, Game};
    use crate::rng::Rng;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn short_game_fingerprint(name: &str, seed: u64) -> String {
        let mut game = Game::new_full(2, 24, 16, seed, 30, 0, false);
        let mut ais = (0..game.players.len())
            .map(|pid| builtin_ai(name, seed.wrapping_add(pid as u64)))
            .collect::<Vec<_>>();
        while game.winner.is_none() {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        serde_json::to_string(&game).expect("serialize deterministic game fingerprint")
    }

    /// A checkout with no trained artifacts is the default state of this
    /// repository — `evolved/` is generated and ignored — so every learned
    /// name must report the scripted agent it really is.
    #[test]
    fn a_bare_checkout_reports_the_agent_that_actually_plays() {
        let dir = "target/test-provenance-bare";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for (name, effective) in [
            ("evolved", "advanced"),
            ("advanced_evolved", "advanced"),
            ("neural", "basic"),
            ("policy", "advanced"),
            ("strategic", "strategic_score"),
        ] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, effective, "{name}");
            assert!(resolved.degraded(), "{name}");
            assert!(resolved.untrained(), "{name}");
            assert!(resolved.line().contains("missing"), "{}", resolved.line());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// The scripted names promise nothing they load, so they are never
    /// degraded and never untrained — including on a bare checkout.
    #[test]
    fn scripted_names_are_never_degraded() {
        let dir = "target/test-provenance-scripted";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for name in ["advanced", "advanced_v1", "basic", "random"] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, name);
            assert!(!resolved.degraded(), "{name}");
            assert!(!resolved.untrained(), "{name}");
        }
        // The evaluator-only control refuses a net by construction, so a
        // missing net is not a degradation for it — only the genome is.
        let control = builtin_provenance("strategic_score", dir);
        assert_eq!(control.effective, "strategic_score");
        assert!(!control.degraded());
        assert!(control.untrained());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn strict_construction_refuses_degraded_names_and_unknown_names() {
        let dir = "target/test-strict-builtin-arms";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();

        let error = strict_builtin_arm_in("policy", dir)
            .expect_err("a bare artifact tier must not construct the learned policy");
        assert_eq!(error.requested(), "policy");
        let provenance = error
            .provenance()
            .expect("a known degraded arm reports its provenance");
        assert!(provenance.degraded());
        assert_eq!(provenance.effective, "advanced");
        assert_eq!(provenance.missing(), vec![CHAMPION_FILE, VALUENET_FILE]);

        let unknown = strict_builtin_arm_in("not-a-selectable-arm", dir)
            .expect_err("strict construction must reject an unknown name");
        assert!(matches!(unknown, BuiltinAiBuildError::UnknownName { .. }));
        assert!(strict_builtin_arm_in("advanced", dir).is_ok());
        fs::remove_dir_all(dir).unwrap();

        builtin_ai_strict("basic", 78_000_090)
            .expect("a production scripted arm must construct strictly");
        let _ = builtin_ai_degraded("not-a-selectable-arm", 78_000_091);
    }

    /// Presence is decided by the loaders the agents use. A file that exists
    /// but cannot load leaves the agent scripted, so provenance must not
    /// call it found.
    #[test]
    fn an_unloadable_artifact_is_not_a_loaded_one() {
        let dir = "target/test-provenance-corrupt";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(format!("{dir}/{VALUENET_FILE}"), "{\"sizes\":[1,2]}").unwrap();
        fs::write(format!("{dir}/{CHAMPION_FILE}"), "not json").unwrap();
        let resolved = builtin_provenance("neural", dir);
        assert_eq!(resolved.effective, "basic");
        assert_eq!(resolved.missing(), vec![CHAMPION_FILE, VALUENET_FILE]);
        fs::remove_dir_all(dir).unwrap();
    }

    /// A missing net changes the controller, not its already-loaded genome.
    /// The effective identity must retain both or the self-comparison guard
    /// warns on distinct agents and misses identical ones.
    #[test]
    fn champion_weighted_fallbacks_keep_the_genome_in_their_identity() {
        let dir = "target/test-provenance-champion-fallback";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::copy(
            format!("data/evolved/{CHAMPION_FILE}"),
            format!("{dir}/{CHAMPION_FILE}"),
        )
        .expect("the committed champion fixture must be available");

        for (name, effective) in [
            ("neural", "basic_evolved"),
            ("policy", "advanced_evolved"),
            ("policy_wide", "advanced_evolved"),
            ("policy_wide_frozen", "advanced_evolved"),
        ] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, effective, "{name}");
            assert!(resolved.degraded(), "{name}");
            assert!(resolved.untrained(), "{name}");
            assert_eq!(resolved.missing(), vec![VALUENET_FILE], "{name}");
        }

        let evolved_alias = builtin_provenance("evolved", dir);
        assert_eq!(evolved_alias.effective, "advanced_evolved");
        assert!(!evolved_alias.degraded(), "a loaded alias is not degraded");
        assert_eq!(
            collapsed_entrants(&["evolved", "advanced_evolved"], dir),
            vec![(
                "evolved".to_string(),
                "advanced_evolved".to_string(),
                "advanced_evolved"
            )]
        );

        assert!(collapsed_entrants(&["neural", "basic"], dir).is_empty());
        assert_eq!(
            collapsed_entrants(&["neural", "basic_evolved"], dir),
            vec![(
                "neural".to_string(),
                "basic_evolved".to_string(),
                "basic_evolved"
            )]
        );
        assert_eq!(
            collapsed_entrants(&["policy", "advanced_evolved"], dir),
            vec![(
                "policy".to_string(),
                "advanced_evolved".to_string(),
                "advanced_evolved"
            )]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// Production resolves the committed embedded champion even when there is
    /// no generated `evolved/` directory. Pin that real resolution path, not
    /// only the useful-but-impossible bare-directory fixture above.
    #[test]
    fn production_fallback_identity_includes_the_embedded_champion() {
        assert!(
            crate::evolve::load_champion(ARTIFACT_DIR).is_some(),
            "the production artifact tier must resolve the committed champion"
        );
        for (name, effective) in [
            ("neural", "basic_evolved"),
            ("policy", "advanced_evolved"),
            ("policy_wide", "advanced_evolved"),
            ("policy_wide_frozen", "advanced_evolved"),
        ] {
            assert_eq!(
                builtin_provenance(name, ARTIFACT_DIR).effective,
                effective,
                "{name}"
            );
        }
        assert!(collapsed_entrants(&["neural", "basic"], ARTIFACT_DIR).is_empty());
        assert_eq!(
            collapsed_entrants(&["policy", "advanced_evolved"], ARTIFACT_DIR),
            vec![(
                "policy".to_string(),
                "advanced_evolved".to_string(),
                "advanced_evolved"
            )]
        );
        assert_eq!(
            collapsed_entrants(
                &["advanced_banking_dedication", "advanced_evolved"],
                ARTIFACT_DIR
            )[0]
                .2,
            "advanced_evolved"
        );
        assert_eq!(
            collapsed_entrants(&["strategic_warm", "strategic"], ARTIFACT_DIR)[0].2,
            "strategic_score"
        );
    }

    #[test]
    fn typed_specs_keep_aliases_collapsed_and_components_visible() {
        let policy = builtin_arm("policy").expect("policy is selectable");
        let evolved = builtin_arm("advanced_evolved").expect("control is selectable");
        assert_eq!(policy.spec, evolved.spec);

        let stock = builtin_arm("advanced").expect("stock is selectable");
        assert_eq!(
            evolved.spec.differing_axes(&stock.spec),
            vec!["weights"],
            "the champion comparison is a one-axis control"
        );

        let economy = builtin_arm("advanced_envoy_economy").expect("arm is selectable");
        assert_eq!(
            economy.spec.differing_axes(&stock.spec),
            vec!["policy-deck-live", "envoy-influence", "envoy-infrastructure"],
            "a composite must expose every changed treatment component"
        );

        let league_top = builtin_arm("advanced_league_top").expect("arm is selectable");
        assert_eq!(
            league_top.spec.weights,
            WeightSource::League,
            "the committed roster has an active Advanced top weight source"
        );
    }

    #[test]
    fn every_selectable_typed_arm_has_a_factory_and_matching_provenance() {
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let arm = builtin_arm(name).expect("every selectable name has a typed arm");
            let _ = arm.build(78_000_100);
            assert_eq!(
                builtin_provenance(name, ARTIFACT_DIR).effective,
                arm.spec.canonical,
                "provenance and factory disagree for {name}"
            );
        }
    }

    #[test]
    fn every_distinct_same_family_arm_declares_a_semantic_axis() {
        let arms = BUILTIN_AIS
            .iter()
            .chain(EVAL_ONLY_AIS.iter())
            .map(|name| (*name, builtin_arm(name).expect("every selectable name has a typed arm")))
            .collect::<Vec<_>>();
        for (index, (left_name, left)) in arms.iter().enumerate() {
            for (right_name, right) in arms.iter().skip(index + 1) {
                let axes = left.spec.differing_axes(&right.spec);
                assert!(
                    !axes.contains(&"implementation"),
                    "{left_name} and {right_name} need an explicit treatment or source axis: {axes:?}"
                );
            }
        }
    }

    #[test]
    fn production_aliases_play_as_their_typed_specs_claim() {
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let arm = builtin_arm(name).expect("every selectable name has a typed arm");
            let effective = arm.spec.canonical;
            if *name == effective {
                continue;
            }
            for seed in [78_000_101, 78_000_102] {
                assert_eq!(
                    short_game_fingerprint(name, seed),
                    short_game_fingerprint(effective, seed),
                    "{name} does not play as its typed canonical arm {effective} on seed {seed}"
                );
            }
        }
    }

    /// Two entrants that resolve to one agent make their difference noise.
    #[test]
    fn entrants_that_collapse_to_one_agent_are_reported() {
        let dir = "target/test-provenance-collapse";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        let collapsed = collapsed_entrants(&["policy", "advanced", "basic"], dir);
        assert_eq!(
            collapsed,
            vec![("policy".to_string(), "advanced".to_string(), "advanced")]
        );
        assert!(collapsed_entrants(&["advanced", "basic", "random"], dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    /// Every selectable entrant name must have an explicit provenance row,
    /// so adding a builtin cannot quietly inherit the catch-all. The
    /// catch-all reports "no artifacts required", which for a learned
    /// entrant is a false statement rather than a missing one — exactly
    /// what this module exists to prevent, and it happened once
    /// (`policy_wide`) before this assertion was tightened.
    #[test]
    fn every_selectable_name_resolves_to_itself_or_a_named_fallback() {
        let dir = "target/test-provenance-names";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS.iter()) {
            let resolved = builtin_provenance(name, dir);
            assert!(
                BUILTIN_AIS.contains(&resolved.effective)
                    || EVAL_ONLY_AIS.contains(&resolved.effective),
                "{name} resolved to unknown {}",
                resolved.effective
            );
            // Only the genuinely scripted agents may report no artifacts.
            // Anything else reaching that state fell through to the
            // catch-all and is claiming to need nothing while quietly
            // needing a net.
            const SCRIPTED: [&str; 45] = [
                "advanced",
                "advanced_belief_pressure",
                "advanced_policy_live_control",
                "advanced_envoy_policy",
                "advanced_envoy_infrastructure",
                "advanced_envoy_priority",
                "advanced_envoy_economy",
                "advanced_strategic_commitment",
                "advanced_blind_to_leaders",
                "advanced_rush",
                "advanced_rush_connected",
                "advanced_congress_counter",
                "advanced_congress_votes",
                "advanced_congress_counter_hard",
                "advanced_counter_in_lane",
                "advanced_counter_stand_down",
                "advanced_early_score_alarm",
                "advanced_early_score_build",
                "advanced_settler_commit",
                "advanced_wide_opening",
                "advanced_expansion_payback",
                "advanced_late_expansion",
                "advanced_expansion_dispatch",
                "advanced_expansion_complete",
                "advanced_city_strategy",
                "advanced_city_strategy_emphasis",
                "advanced_city_strategy_roles",
                "advanced_city_strategy_roles_raw",
                "advanced_city_strategy_raw",
                "advanced_city_strategy_bastion_only",
                "advanced_city_strategy_breadbasket_only",
                "advanced_city_strategy_comparative_only",
                "advanced_city_strategy_pressure_only",
                "advanced_civ_blind",
                "advanced_food_first",
                "advanced_lane_reachable",
                "advanced_league_top",
                "advanced_parallel_settlers",
                "advanced_prophet_first",
                "advanced_joint_tactics",
                "advanced_relief_scoped",
                "advanced_settler_first",
                "advanced_v1",
                "basic",
                "random",
            ];
            assert!(
                !resolved.artifacts.is_empty() || SCRIPTED.contains(name),
                "{name} has no provenance row and inherited the catch-all"
            );
            // The whitelist above is a list of names, so it grows every time
            // a scripted entrant is added and stops discriminating as it
            // does. This does not: the catch-all answers `basic`, so any
            // name that needs no artifacts and still does not resolve to
            // itself reached that arm rather than a row of its own.
            if resolved.artifacts.is_empty() {
                assert_eq!(
                    resolved.effective, *name,
                    "{name} needs no artifacts yet resolves to {}, which only \
                     the catch-all does",
                    resolved.effective
                );
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// The reactor experiment pins generation 14 inside its dedicated runner.
    /// A public factory once reconstructed this name with default weights,
    /// silently changing the controller under treatment.
    #[test]
    fn reactor_marginal_treatment_stays_private_to_its_pinned_runner() {
        const NAME: &str = "advanced_reactor_marginal";
        assert!(!BUILTIN_AIS.contains(&NAME));
        assert!(!EVAL_ONLY_AIS.contains(&NAME));
        assert_eq!(builtin_provenance(NAME, "unused").effective, "basic");
    }

    #[test]
    fn static_doctrine_challengers_construct_searching_agents() {
        for name in [
            "strategic_deep_expand",
            "strategic_deep_consolidate",
            "strategic_deep_militarize",
        ] {
            let ai = builtin_ai(name, 1);
            assert_eq!(ai.review_census(), Some(Default::default()), "{name}");
        }
    }

    #[test]
    fn joint_axis_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_joint", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_joint", "unused");
        assert_eq!(provenance.effective, "strategic_joint");
        assert!(!provenance.degraded());
    }

    #[test]
    fn terminal_tempo_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_tempo", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_tempo", "unused");
        assert_eq!(provenance.effective, "strategic_deep_tempo");
        assert!(!provenance.degraded());
    }

    #[test]
    fn ultra_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_ultra", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_ultra", "unused");
        assert_eq!(provenance.effective, "strategic_ultra");
        assert!(!provenance.degraded());
    }

    #[test]
    fn deep_default_control_refuses_the_champion_artifact() {
        let ai = builtin_ai("strategic_deep_default", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_default", "unused");
        assert_eq!(provenance.effective, "strategic_deep_default");
        assert!(!provenance.degraded());
        assert!(
            provenance
                .artifacts
                .iter()
                .all(|artifact| artifact.file != CHAMPION_FILE),
            "the control must never resolve best.json"
        );
    }

    #[test]
    fn religious_conversion_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_conversion", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_conversion", "unused");
        assert_eq!(provenance.effective, "strategic_deep_conversion");
        assert!(!provenance.degraded());
    }

    #[test]
    fn religious_checkmate_challenger_constructs_a_searching_agent() {
        let ai = builtin_ai("strategic_deep_checkmate", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_checkmate", "unused");
        assert_eq!(provenance.effective, "strategic_deep_checkmate");
        assert!(!provenance.degraded());
    }

    #[test]
    fn league_genome_challenger_loads_a_win_selected_searching_agent() {
        let (name, _) = league_generalist().expect("committed league has a generalist genome");
        assert_eq!(name, "g20-21", "update the documented transfer candidate");
        let ai = builtin_ai("strategic_deep_league", 1);
        assert_eq!(ai.review_census(), Some(Default::default()));
        let provenance = builtin_provenance("strategic_deep_league", "unused");
        assert_eq!(provenance.effective, "strategic_deep_league");
        assert!(!provenance.degraded());
        assert!(!provenance.untrained());
    }

    fn player(name: &str, leader: &str, civ: &str, score: i64, won: bool) -> RatedPlayer {
        RatedPlayer::new(name, leader, civ, score, won)
    }

    #[test]
    fn win_shares_are_a_distribution_over_the_table() {
        let table = [1914.0, 1865.0, 1836.0, 1847.0, 1766.0, 1755.0];
        let shares = win_shares(&table);
        assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(shares[0] > shares[5]);
        let pair = win_shares(&[1600.0, 1400.0]);
        assert!((pair[0] - expected(1600.0, 1400.0)).abs() < 1e-12);
        let wide = win_shares(&[40_000.0, 0.0]);
        assert!((wide[0] + wide[1] - 1.0).abs() < 1e-9 && wide[0] > 0.999);
    }

    #[test]
    fn direct_pair_score_interval_is_bounded_and_symmetric() {
        let (low, high) = wilson_interval(31.0, 40);
        let (reverse_low, reverse_high) = wilson_interval(9.0, 40);
        assert!(low < 31.0 / 40.0 && 31.0 / 40.0 < high);
        assert!((low - (1.0 - reverse_high)).abs() < 1e-12);
        assert!((high - (1.0 - reverse_low)).abs() < 1e-12);
        assert_eq!(wilson_interval(0.0, 0), (0.0, 1.0));
        assert_eq!(performance_elo(1500.0, 0.5), 1500.0);
        assert!(performance_elo(1500.0, 0.0).is_infinite());
        assert!(performance_elo(1500.0, 1.0).is_infinite());
        assert!(
            (performance_elo(1500.0, low) + performance_elo(1500.0, reverse_high)
                - 3000.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn result_updates_player_leader_civilization_rows() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("TechPriest", "Trajan", "Rome", 200, true),
                player("LabRat", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        let rome = &pool.ratings[&RatingKey::new("TechPriest", "Trajan", "Rome")];
        let egypt = &pool.ratings[&RatingKey::new("LabRat", "Cleopatra", "Egypt")];
        assert_eq!(rome.elo, 1012.0);
        assert_eq!(egypt.elo, 988.0);
        assert_eq!((rome.games, rome.wins), (1, 1));
        assert_eq!(pool.overall["TechPriest"], *rome);
        assert_eq!(pool.overall["LabRat"], *egypt);
    }

    #[test]
    fn immutable_control_pins_the_longitudinal_scale() {
        let mut cfg = TourneyCfg {
            players_per_game: 2,
            rating_anchor: Some("Control".to_string()),
            ..TourneyCfg::default()
        };
        cfg.num_city_states = 0;
        let mut anchored = EloPool::with_base(1500.0);
        anchored
            .bind_profile(TournamentProfile::from_cfg(&cfg))
            .unwrap();
        anchored.record_game(
            &[
                player("Challenger", "Trajan", "Rome", 200, true),
                player("Control", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );

        assert!((anchored.overall["Control"].elo - 1500.0).abs() < 1e-12);
        assert!((anchored.overall["Challenger"].elo - 1524.0).abs() < 1e-12);
        assert!(anchored
            .ratings
            .values()
            .any(|rating| (rating.elo - 1524.0).abs() < 1e-12));

        // Evidence about the fixed control translates every older row. It
        // cannot move the anchor or inflate only the newest generation.
        anchored.record_game(
            &[
                player("Control", "Cleopatra", "Egypt", 200, true),
                player("Novice", "Pericles", "Greece", 100, false),
            ],
            24.0,
        );
        assert!((anchored.overall["Control"].elo - 1500.0).abs() < 1e-12);
        assert!((anchored.overall["Challenger"].elo - 1512.0).abs() < 1e-12);
        assert!((anchored.overall["Novice"].elo - 1476.0).abs() < 1e-12);

        let direct = direct_anchor_performance(&anchored, "Control");
        let challenger = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "Challenger")
            .unwrap();
        let novice = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "Novice")
            .unwrap();
        assert_eq!((challenger.2, challenger.3), (1.0, 1));
        assert_eq!((novice.2, novice.3), (0.0, 1));
        assert!(challenger.1 > 1500.0 && novice.1 < 1500.0);

        let complete_report = leaderboard(&anchored);
        assert!(complete_report.contains("Standardized direct performance Elo"));
        let mut migrated = anchored.clone();
        migrated.history_complete = false;
        let migrated_report = leaderboard(&migrated);
        assert!(migrated_report.contains(
            "Post-migration direct performance (retained raw games only; legacy prior excluded)"
        ));
        assert!(!migrated_report.contains("Standardized direct performance Elo"));
    }

    #[test]
    fn overall_rating_accumulates_across_civilizations() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 100, false),
                player("Bob", "Cleopatra", "Egypt", 200, true),
            ],
            24.0,
        );

        let alice = &pool.overall["Alice"];
        assert_eq!((alice.games, alice.wins), (2, 1));
        assert!(alice.elo < 1000.0, "the upset must erase more than the first win added");
        assert_eq!(
            alice.elo,
            pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo
        );
        assert_eq!(
            pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")].games,
            1
        );
    }

    #[test]
    fn a_new_combination_inherits_its_players_global_rating() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 200, true),
                player("Bob", "Pericles", "Greece", 100, false),
            ],
            0.0,
        );

        assert_eq!(
            pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo,
            1012.0
        );
        assert_eq!(
            pool.ratings[&RatingKey::new("Bob", "Pericles", "Greece")].elo,
            988.0
        );
    }

    #[test]
    fn cloned_seats_count_as_one_overall_game_and_one_player_pair() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 400, true),
                player("Alice", "Pericles", "Greece", 300, false),
                player("Bob", "Cleopatra", "Egypt", 200, false),
                player("Bob", "Qin Shi Huang", "China", 100, false),
            ],
            24.0,
        );

        assert_eq!(pool.overall["Alice"].elo, 1012.0);
        assert_eq!(pool.overall["Bob"].elo, 988.0);
        assert_eq!((pool.overall["Alice"].games, pool.overall["Alice"].wins), (1, 1));
        assert_eq!((pool.overall["Bob"].games, pool.overall["Bob"].wins), (1, 0));
    }

    #[test]
    fn a_ledger_rejects_a_different_tournament_profile() {
        let cfg = TourneyCfg::default();
        let original = TournamentProfile::from_cfg(&cfg);
        let mut changed_cfg = TourneyCfg::default();
        changed_cfg.width += 2;
        let changed = TournamentProfile::from_cfg(&changed_cfg);
        let mut pool = EloPool::with_base(1000.0);

        pool.bind_profile(original.clone()).unwrap();
        let error = pool.bind_profile(changed).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("rating profile mismatch"));
        assert_eq!(pool.profile, Some(original));

        let mut controller_changed = pool.profile.clone().unwrap();
        controller_changed.controller_roster =
            ["advanced", "advanced_v1", "basic", "strategic"]
                .into_iter()
                .map(str::to_string)
                .collect();
        let error = pool.bind_profile(controller_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));

        let mut rules_changed = pool.profile.clone().unwrap();
        rules_changed.rules_fingerprint = "fnv1a64:0000000000000000".to_string();
        let error = pool.bind_profile(rules_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));

        let mut setup_changed = pool.profile.clone().unwrap();
        setup_changed.setup_contract = "difficulty=deity".to_string();
        let error = pool.bind_profile(setup_changed).unwrap_err();
        assert!(error.to_string().contains("rating profile mismatch"));
    }

    #[test]
    fn persistent_elo_pins_every_implicit_lobby_default() {
        assert_eq!(
            super::tournament_setup_contract(&TourneyCfg::default()),
            super::SCHEMA3_LEGACY_SETUP_CONTRACT
        );
    }

    #[test]
    fn old_schema_three_profiles_migrate_to_the_historical_lobby() {
        let mut encoded =
            serde_json::to_value(TournamentProfile::from_cfg(&TourneyCfg::default())).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("setup_contract");

        let migrated: TournamentProfile = serde_json::from_value(encoded).unwrap();

        assert_eq!(
            migrated.setup_contract,
            super::SCHEMA3_LEGACY_SETUP_CONTRACT
        );
    }

    #[test]
    fn persistent_ratings_reject_cloned_or_duplicate_entrants() {
        let cfg = TourneyCfg::default();
        let make = |_: &str, _: u64| {
            Box::new(crate::ai::BasicAi::new()) as Box<dyn crate::ai::Ai>
        };
        let too_few = vec!["advanced".to_string(), "basic".to_string()];
        let error = super::run_persistent_tournament(
            &too_few,
            make,
            &cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("cloned seats change the contest"));

        let duplicate = vec![
            "advanced".to_string(),
            "basic".to_string(),
            "basic".to_string(),
            "random".to_string(),
        ];
        let error = super::run_persistent_tournament(
            &duplicate,
            make,
            &cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be unique"));

        let anchored_cfg = TourneyCfg {
            rating_anchor: Some("missing-control".to_string()),
            ..TourneyCfg::default()
        };
        let distinct = vec![
            "advanced".to_string(),
            "advanced_v1".to_string(),
            "basic".to_string(),
            "random".to_string(),
        ];
        let error = super::run_persistent_tournament(
            &distinct,
            make,
            &anchored_cfg,
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be one of the tournament entrants"));

        let error = super::run_persistent_tournament(
            &distinct,
            make,
            &TourneyCfg::default(),
            "target/elo-test-must-not-exist.json",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("one ordered controller role per entrant"));
    }

    #[test]
    fn score_ties_are_draws_and_still_count_as_games() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 150, false),
                player("Bob", "Cleopatra", "Egypt", 150, false),
            ],
            24.0,
        );
        for rating in pool.ratings.values() {
            assert_eq!(rating.elo, 1000.0);
            assert_eq!(rating.games, 1);
            assert_eq!(rating.wins, 0);
        }
    }

    #[test]
    fn a_player_has_independent_ratings_for_different_leaders() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 100, false),
                player("Bob", "Cleopatra", "Egypt", 200, true),
            ],
            24.0,
        );
        let trajan = &pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")];
        let eleanor = &pool.ratings[&RatingKey::new("Alice", "Eleanor", "England")];
        assert_eq!(trajan.games, 1);
        assert_eq!(eleanor.games, 1);
        assert!(trajan.elo > 1000.0);
        assert!(eleanor.elo < 1000.0);
    }

    #[test]
    fn declared_winner_outranks_a_higher_score() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Trajan", "Rome", 80, true),
                player("Bob", "Cleopatra", "Egypt", 200, false),
            ],
            24.0,
        );
        assert!(pool.ratings[&RatingKey::new("Alice", "Trajan", "Rome")].elo > 1000.0);
    }

    #[test]
    fn eleanor_leading_two_civilizations_has_two_ratings() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 200, true),
                player("Bob", "Victoria", "England", 100, false),
            ],
            24.0,
        );
        pool.record_game(
            &[
                player("Alice", "Eleanor", "France", 100, false),
                player("Bob", "Catherine de Medici", "France", 200, true),
            ],
            24.0,
        );
        assert!(pool
            .ratings
            .contains_key(&RatingKey::new("Alice", "Eleanor", "England")));
        assert!(pool
            .ratings
            .contains_key(&RatingKey::new("Alice", "Eleanor", "France")));
        assert!(pool.ratings[&RatingKey::new("Alice", "Eleanor", "England")].elo > 1000.0);
        assert!(pool.ratings[&RatingKey::new("Alice", "Eleanor", "France")].elo < 1000.0);
    }

    #[test]
    fn one_player_cannot_rate_their_leaders_against_each_other() {
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("Alice", "Eleanor", "England", 200, true),
                player("Alice", "Eleanor", "France", 100, false),
            ],
            24.0,
        );
        assert!(pool.ratings.values().all(|rating| rating.elo == 1000.0));
        assert!(pool.ratings.values().all(|rating| rating.games == 1));
    }

    #[test]
    fn round_robin_scheduler_balances_every_entrant_across_civilization_seats() {
        let names: Vec<String> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let mut rng = Rng::new(9);
        let (order, stride) = seat_schedule(&names, 4, &mut rng);
        let mut appearances = BTreeMap::<String, u32>::new();
        let mut by_seat = vec![BTreeMap::<String, u32>::new(); 4];
        for game in 0..25 {
            let seats = scheduled_seats(&names, 4, game, &order, stride);
            assert_eq!(
                seats
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                4
            );
            for (seat, entrant) in seats.into_iter().enumerate() {
                *appearances.entry(entrant.clone()).or_insert(0) += 1;
                *by_seat[seat].entry(entrant).or_insert(0) += 1;
            }
        }
        assert_eq!(appearances.values().sum::<u32>(), 100);
        assert!(appearances.values().all(|count| *count == 20));
        for seat in by_seat {
            assert_eq!(seat.len(), names.len());
            assert!(seat.values().all(|count| *count == 5));
        }
    }

    #[test]
    fn ledger_round_trips_structured_keys() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("civvis-elo-{}-{nonce}", std::process::id()));
        let path = dir.join("ratings.json");
        let mut pool = EloPool::with_base(1000.0);
        pool.record_game(
            &[
                player("TechPriest", "Trajan", "Rome", 2, true),
                player("CultureVulture", "Cleopatra", "Egypt", 1, false),
            ],
            24.0,
        );
        pool.save(&path).unwrap();
        assert_eq!(EloPool::load(&path).unwrap(), pool);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(&format!("\"schema_version\": {ELO_SCHEMA_VERSION}")));
        assert!(raw.contains("\"civilization\": \"Rome\""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keyed_games_are_idempotent_and_independent_of_lock_order() {
        let first_game = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Bob", "Cleopatra", "Egypt", 100, false),
        ];
        let second_game = [
            player("Alice", "Eleanor", "France", 100, false),
            player("Bob", "Pericles", "Greece", 200, true),
        ];
        let mut reverse_arrival = EloPool::with_base(1500.0);
        reverse_arrival
            .record_game_once("event-b", &second_game, 24.0)
            .unwrap();
        reverse_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap();
        let mut forward_arrival = EloPool::with_base(1500.0);
        forward_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap();
        forward_arrival
            .record_game_once("event-b", &second_game, 24.0)
            .unwrap();

        assert_eq!(reverse_arrival, forward_arrival);
        assert!(!forward_arrival
            .record_game_once("event-a", &first_game, 24.0)
            .unwrap());
        let mut changed = first_game.to_vec();
        changed[0].score += 1;
        let error = forward_arrival
            .record_game_once("event-a", &changed, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("different results"));
    }

    #[test]
    fn keyed_games_refuse_cloned_identities_and_profile_shape_drift() {
        let clones = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Alice", "Pericles", "Greece", 100, false),
        ];
        let mut unbound = EloPool::with_base(1500.0);
        let error = unbound
            .record_game_once("cloned-table", &clones, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("identities must be distinct"));

        let cfg = TourneyCfg::default();
        let mut profiled = EloPool::with_base(1500.0);
        profiled
            .bind_profile(TournamentProfile::from_cfg(&cfg))
            .unwrap();
        let duel = [
            player("Alice", "Trajan", "Rome", 200, true),
            player("Bob", "Cleopatra", "Egypt", 100, false),
        ];
        let error = profiled
            .record_game_once("wrong-table-size", &duel, 24.0)
            .unwrap_err();
        assert!(error.to_string().contains("match the profile"));
    }

    #[test]
    fn complete_history_detects_a_tampered_aggregate() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "civvis-elo-tamper-{}-{nonce}",
            std::process::id()
        ));
        let path = dir.join("ratings.json");
        let mut pool = EloPool::with_base(1500.0);
        pool.record_game_once(
            "event-a",
            &[
                player("Alice", "Trajan", "Rome", 200, true),
                player("Bob", "Cleopatra", "Egypt", 100, false),
            ],
            24.0,
        )
        .unwrap();
        pool.save(&path).unwrap();

        let mut stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let elo = stored["players"][0]["elo"].as_f64().unwrap();
        stored["players"][0]["elo"] = serde_json::Value::from(elo + 5.0);
        fs::write(&path, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
        let error = EloPool::load(&path).unwrap_err();
        assert!(error
            .to_string()
            .contains("aggregates do not match raw game evidence"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn historical_protocol_v1_ledger_is_preserved() {
        let pool = EloPool::load(HISTORICAL_V1_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        let mut historical_profile = TournamentProfile::from_cfg(&expected_cfg);
        historical_profile.protocol_version = 1;
        assert_eq!(
            pool.profile,
            Some(historical_profile)
        );
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260730",
                "advanced_v1",
                "basic-20260730",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260730")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (31.0, 40));
        assert!((advanced.1 - 1708.2).abs() < 0.1);
        assert!((100.0 * advanced.4 - 62.5).abs() < 0.1);
        assert!((100.0 * advanced.5 - 87.7).abs() < 0.1);
    }

    #[test]
    fn shipped_protocol_v2_ledger_is_a_canonical_fresh_baseline() {
        let pool = EloPool::load(DEFAULT_RATINGS_PATH).unwrap();
        let expected_cfg = TourneyCfg {
            rating_anchor: Some("advanced_v1".to_string()),
            controller_roster: ["advanced", "advanced_v1", "basic", "random"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            ..TourneyCfg::default()
        };
        assert_eq!(pool.base_rating, ELO_BASE_RATING);
        assert_eq!(
            pool.profile,
            Some(TournamentProfile::from_cfg(&expected_cfg))
        );
        assert!(pool.history_complete);
        assert_eq!(pool.history.len(), 40);
        assert!(pool.history.iter().all(|game| {
            game.id
                .as_deref()
                .is_some_and(|id| id.starts_with("v2:"))
        }));
        assert_eq!(
            pool.history.len(),
            pool.overall
                .values()
                .map(|rating| rating.games)
                .max()
                .unwrap_or(0) as usize
        );
        assert_eq!(
            pool.overall.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "advanced-20260731",
                "advanced_v1",
                "basic-20260730",
                "random-20260730",
            ]
        );
        assert_eq!(pool.ratings.len(), 16);

        let direct = direct_anchor_performance(&pool, "advanced_v1");
        let advanced = direct
            .iter()
            .find(|(player, _, _, _, _, _)| player == "advanced-20260731")
            .unwrap();
        assert_eq!((advanced.2, advanced.3), (27.0, 40));
        assert!((advanced.1 - 1623.6).abs() < 0.1);
        assert!((100.0 * advanced.4 - 52.0).abs() < 0.1);
        assert!((100.0 * advanced.5 - 79.9).abs() < 0.1);
    }

    #[test]
    fn schema_one_rows_migrate_to_player_leader_civilization() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("civvis-elo-migrate-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ratings.json");
        fs::write(
            &path,
            r#"{"schema_version":1,"base_rating":1000.0,"ratings":[{"civilization":"Rome","strategy":"science","elo":1111.0,"games":3,"wins":2,"agents":["advanced"]}]}"#,
        )
        .unwrap();
        let pool = EloPool::load(&path).unwrap();
        let rating = &pool.ratings[&RatingKey::new("advanced", "Trajan", "Rome")];
        assert_eq!((rating.elo, rating.games, rating.wins), (1111.0, 3, 2));
        assert_eq!(pool.overall["advanced"].elo, rating.elo);
        assert_eq!((pool.overall["advanced"].games, pool.overall["advanced"].wins), (0, 0));
        assert!(!pool.history_complete);
        pool.save(&path).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(&format!("\"schema_version\": {ELO_SCHEMA_VERSION}")));
        assert!(raw.contains("\"players\""));
        assert!(!raw.contains("\"strategy\""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn locked_ledger_updates_from_concurrent_workers_are_merged() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "civvis-elo-concurrent-{}-{nonce}",
            std::process::id()
        ));
        let path = dir.join("ratings.json");
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = [
            (
                "event-b",
                player("TechPriest", "Trajan", "Rome", 2, true),
                player("LabRat", "Cleopatra", "Egypt", 1, false),
            ),
            (
                "event-a",
                player("CultureVulture", "Pericles", "Greece", 2, true),
                player("OperaGhost", "Qin Shi Huang", "China", 1, false),
            ),
        ]
        .into_iter()
        .map(|results| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                super::update_ledger(&path, |pool| {
                    pool.record_game_once(results.0, &[results.1, results.2], 24.0)
                        .unwrap();
                })
                .unwrap();
            })
        })
        .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let pool = EloPool::load(&path).unwrap();
        assert!(pool.history_complete);
        assert_eq!(
            pool.history
                .iter()
                .map(|game| game.id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["event-a", "event-b"]
        );
        assert_eq!(pool.ratings.len(), 4);
        assert_eq!(
            pool.ratings
                .values()
                .map(|rating| rating.games)
                .sum::<u32>(),
            4
        );
        assert!(!dir.join(".ratings.json.lock").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
