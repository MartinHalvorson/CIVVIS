//! Elo tournament harness: evaluate AI strategies against each other.
//!
//! Every rating belongs to one `(player, leader, civilization)` combination.
//! A player may be a human account or a named AI strategy; changing leaders
//! selects a different rating without changing player identity. Leader and
//! civilization are both retained because they are not one-to-one (Eleanor,
//! for example, can lead either England or France).
//! Multiplayer games are scored as pairwise results with `K/(n-1)` scaling.
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi, Weights};
use crate::game::{Action, Game};
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::MapSize;

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
pub const EVAL_ONLY_AIS: [&str; 40] = [
    "advanced_banking_dedication",
    "advanced_blind_to_leaders",
    "advanced_civ_blind",
    "advanced_counter_in_lane",
    "advanced_counter_stand_down",
    "advanced_settler_commit",
    "advanced_food_first",
    "advanced_measured_dedication",
    "advanced_lane_reachable",
    "advanced_parallel_settlers",
    "advanced_relief_scoped",
    "strategic_score",
    "strategic_doctrine",
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
    "strategic_cold",
    "strategic_noprophet",
    "strategic_deep_adaptive",
    "strategic_rivals",
    "strategic_deep_rivals",
];

/// On-disk schema for the shared player/leader/civilization rating ledger.
pub const ELO_SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_RATINGS_PATH: &str = "data/elo_ratings.json";
const LEAGUE_SNAPSHOT_DIR: &str = "data/league";
const LEAGUE_SNAPSHOT_FILE: &str = "data/league/league.json";

/// Conservatively strongest active, untargeted genome in the committed
/// outcome-rated league. Lane specialists answer a different question; this
/// challenger isolates whether a win-selected generalist policy transfers to
/// the strongest macro-search budget.
fn league_generalist() -> Option<(String, Weights)> {
    crate::league::load_league(LEAGUE_SNAPSHOT_DIR)?
        .strategies
        .into_iter()
        .filter(|strategy| !strategy.retired && !strategy.human)
        .filter_map(|strategy| match strategy.kind {
            crate::league::StrategyKind::Advanced {
                weights,
                target: None,
            } => Some((strategy.rating - 1.96 * strategy.rd, strategy.name, weights)),
            _ => None,
        })
        .max_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, name, weights)| (name, weights))
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

impl Rating {
    fn new(base: f64) -> Self {
        Self {
            elo: base,
            games: 0,
            wins: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EloPool {
    pub base_rating: f64,
    /// The rating identity is deliberately structured, not a display string:
    /// player, leader, and civilization can be queried independently.
    pub ratings: BTreeMap<RatingKey, Rating>,
}

#[derive(Serialize, Deserialize)]
struct StoredPool {
    schema_version: u32,
    base_rating: f64,
    ratings: Vec<StoredRating>,
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
#[derive(Clone, Debug, PartialEq)]
pub struct RatedPlayer {
    pub key: RatingKey,
    pub score: i64,
    pub won: bool,
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

impl EloPool {
    /// Keep the historical constructor shape for library callers. Entrants no
    /// longer create rating rows up front because their leader/civilization
    /// combinations are not known until a game has run.
    pub fn new(_names: &[String], base: f64) -> EloPool {
        EloPool {
            base_rating: base,
            ratings: BTreeMap::new(),
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
        if !matches!(stored.schema_version, 1 | ELO_SCHEMA_VERSION) {
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
                if stored.schema_version == ELO_SCHEMA_VERSION {
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
        Ok(EloPool {
            base_rating: stored.base_rating,
            ratings,
        })
    }

    pub fn load_or_new(path: impl AsRef<Path>, base: f64) -> io::Result<EloPool> {
        match Self::load(path) {
            Ok(pool) => Ok(pool),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::with_base(base)),
            Err(error) => Err(error),
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
        for player in players {
            self.ratings
                .entry(player.key.clone())
                .or_insert_with(|| Rating::new(self.base_rating));
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
                let actual_a = if a.won != b.won {
                    if a.won {
                        1.0
                    } else {
                        0.0
                    }
                } else if a.score > b.score {
                    1.0
                } else if a.score < b.score {
                    0.0
                } else {
                    0.5
                };
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

pub fn builtin_ai(name: &str, seed: u64) -> Box<dyn Ai> {
    match name {
        "advanced" => Box::new(AdvancedAi::new()),
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
        // deterrent, and a real cost in development at deployment scale.
        "advanced_blind_to_leaders" => {
            let mut ai = AdvancedAi::new();
            ai.deny_leaders = false;
            Box::new(ai)
        }
        // Treatment for the response-shape axis: identical to `advanced`
        // except that a Science or Expansion threat is answered by racing the
        // leader in that lane rather than by declaring on them. The alarm is
        // unchanged; only what it asks for changes. See
        // `docs/COUNTERING_LEADERS.md`: at deployment scale one or two
        // belligerents wins 4.4% and 10.7% of seats against a 16.7% base.
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
        "advanced_lane_reachable" => {
            let mut ai = AdvancedAi::new();
            ai.refuse_unreachable_lanes = true;
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
        // rounds instead of 40. The strongest configuration measured —
        // Promoted on a pre-registered 300-map run at a fresh seed:
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
        // Static genome challengers for the strongest measured search
        // budget. Unlike `strategic_doctrine`, these do not ask a noisy
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
        // The same opponent-model treatment on the strongest measured macro
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
        _ => Box::new(BasicAi::new()),
    }
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

/// What a builtin name actually plays as once its artifacts are resolved.
///
/// `builtin_ai` falls back silently when a trained artifact is missing —
/// correctly, because a missing file should not stop a game. What it must
/// not do is let an evaluation record the result under the learned name: on
/// a checkout with no `evolved/` directory, `neural` is `basic` and
/// `policy` is `advanced`, so a run pitting them against `advanced`
/// measures the scripted agent against itself and reports it as evidence
/// about a learned one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProvenance {
    /// The name the caller asked for.
    pub requested: String,
    /// Every artifact the name reads, in the order it reads them.
    pub artifacts: Vec<ArtifactStatus>,
    /// The agent that actually plays. Equals `requested` unless a
    /// definitional artifact is missing.
    pub effective: &'static str,
}

impl AgentProvenance {
    /// True when the name promises more than the loaded artifacts deliver.
    pub fn degraded(&self) -> bool {
        self.effective != self.requested
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
            return match self.artifacts.is_empty() {
                true => format!("{}: scripted, no artifacts required", self.requested),
                false => format!("{}: loaded {}", self.requested, self.artifacts_list()),
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

/// Resolve what `builtin_ai(name, _)` will actually construct from `dir`.
///
/// Presence is decided by the same loaders the agents use, not by a stat:
/// a `valuenet.json` that fails `ValueNet::valid` is rejected at load time,
/// so reporting it as present would restate the bug it is meant to catch.
pub fn builtin_provenance(name: &str, dir: &str) -> AgentProvenance {
    let champion = crate::evolve::load_champion(dir).is_some();
    let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
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
    let (artifacts, effective) = match name {
        // The genome *is* these two names; without it they are the stock
        // scripted agent under a name that claims otherwise.
        "evolved" => (
            vec![ArtifactStatus {
                definitional: true,
                ..genome
            }],
            if champion { "evolved" } else { "advanced" },
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
        // NeuralAi needs the net to exist at all and drops all the way to
        // the lightweight agent without it — the largest silent gap here.
        "neural" => (
            vec![genome, value(true)],
            if net { "neural" } else { "basic" },
        ),
        "policy" => (
            vec![genome, value(true)],
            if net { "policy" } else { "advanced" },
        ),
        // The *wide* net is definitional and is a different artifact from
        // the one `policy` wants: `load_width` refuses each to the other,
        // so without a 34-wide net in place this is the scripted agent.
        "policy_wide" | "policy_wide_frozen" => (
            vec![
                genome,
                ArtifactStatus {
                    file: VALUENET_FILE,
                    found: crate::valuenet::ValueNet::load_width(
                        dir,
                        crate::decision_features::WIDTH,
                    )
                    .is_some(),
                    definitional: true,
                },
            ],
            if crate::valuenet::ValueNet::load_width(dir, crate::decision_features::WIDTH).is_some()
            {
                if name == "policy_wide" {
                    "policy_wide"
                } else {
                    "policy_wide_frozen"
                }
            } else {
                "advanced"
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
        "strategic_r20" => (vec![genome, value(false)], "strategic_r20"),
        "strategic_r10" => (vec![genome, value(false)], "strategic_r10"),
        "strategic_nodefer" => (vec![genome, value(false)], "strategic_nodefer"),
        "strategic_r20h20" => (vec![genome, value(false)], "strategic_r20h20"),
        "strategic_h80" => (vec![genome, value(false)], "strategic_h80"),
        "strategic_rot20" => (vec![genome, value(false)], "strategic_rot20"),
        "strategic_warm" => (vec![genome, value(false)], "strategic_warm"),
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
        "advanced_lane_reachable" => (Vec::new(), "advanced_lane_reachable"),
        "advanced_banking_dedication" => (Vec::new(), "advanced_banking_dedication"),
        "advanced_measured_dedication" => (Vec::new(), "advanced_measured_dedication"),
        "advanced_parallel_settlers" => (Vec::new(), "advanced_parallel_settlers"),
        "advanced_blind_to_leaders" => (Vec::new(), "advanced_blind_to_leaders"),
        "advanced_counter_in_lane" => (Vec::new(), "advanced_counter_in_lane"),
        "advanced_counter_stand_down" => (Vec::new(), "advanced_counter_stand_down"),
        "advanced_civ_blind" => (Vec::new(), "advanced_civ_blind"),
        "advanced_settler_commit" => (Vec::new(), "advanced_settler_commit"),
        "advanced_food_first" => (Vec::new(), "advanced_food_first"),
        "advanced_v1" => (Vec::new(), "advanced_v1"),
        "advanced_relief_scoped" => (Vec::new(), "advanced_relief_scoped"),
        "random" => (Vec::new(), "random"),
        // `builtin_ai` answers every other name with the lightweight agent.
        "basic" => (Vec::new(), "basic"),
        _ => (Vec::new(), "basic"),
    };
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
    let resolved = builtin_provenances(names, dir);
    let mut out = Vec::new();
    for (index, left) in resolved.iter().enumerate() {
        for right in resolved.iter().skip(index + 1) {
            if left.requested != right.requested && left.effective == right.effective {
                out.push((
                    left.requested.clone(),
                    right.requested.clone(),
                    left.effective,
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
    pub max_turns: u32,
    pub num_city_states: usize,
    pub seed: u64,
    pub k: f64,
    pub verbose: bool,
    /// How many games to play at once. Results and rating checkpoints remain
    /// in game order, so concurrency does not change the final table.
    pub jobs: usize,
}

impl Default for TourneyCfg {
    fn default() -> Self {
        let size = MapSize::for_players(4);
        TourneyCfg {
            games: 20,
            players_per_game: 4,
            width: size.width,
            height: size.height,
            max_turns: 150,
            num_city_states: size.default_city_states,
            seed: 0,
            k: 24.0,
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
    C: FnMut(&[RatedPlayer]) -> Result<(), E>,
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
        let mut game = Game::new(
            cfg.players_per_game,
            cfg.width,
            cfg.height,
            *gseed,
            cfg.max_turns,
            cfg.num_city_states,
        );
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
            results,
            wname,
            winner.map_or_else(
                || "-".to_string(),
                |winner| game.players[winner].civ.clone(),
            ),
            game.victory_type.clone().unwrap_or_default(),
            game.turn,
        )
    });

    for (game_index, (results, winner, civilization, victory, turn)) in
        played.into_iter().enumerate()
    {
        checkpoint(&results)?;
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
    let mut pool = EloPool::new(names, 1000.0);
    let result: Result<(), std::convert::Infallible> =
        play_tournament(names, &make, cfg, |players| {
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
    let result: Result<(), std::convert::Infallible> =
        play_tournament(names, &make, cfg, |players| {
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

fn update_ledger(path: &Path, update: impl FnOnce(&mut EloPool)) -> io::Result<EloPool> {
    let _lock = acquire_ledger_lock(path)?;
    let mut pool = EloPool::load_or_new(path, 1000.0)?;
    update(&mut pool);
    pool.save(path)?;
    Ok(pool)
}

/// Run a tournament against the latest shared ledger and atomically checkpoint
/// every completed game. The short per-game lock prevents concurrent agents
/// from overwriting one another's updates.
pub fn run_persistent_tournament<F>(
    names: &[String],
    make: F,
    cfg: &TourneyCfg,
    path: impl AsRef<Path>,
) -> io::Result<EloPool>
where
    F: Fn(&str, u64) -> Box<dyn Ai> + Sync,
{
    let path = path.as_ref();
    let mut pool = update_ledger(path, |_| {})?;
    play_tournament(names, &make, cfg, |players| {
        pool = update_ledger(path, |latest| latest.record_game(players, cfg.k))?;
        Ok::<(), io::Error>(())
    })?;
    Ok(pool)
}

pub fn leaderboard(pool: &EloPool) -> String {
    let mut rows: Vec<(&RatingKey, &Rating)> = pool.ratings.iter().collect();
    rows.sort_by(|(key_a, a), (key_b, b)| {
        b.elo
            .total_cmp(&a.elo)
            .then(key_a.player.cmp(&key_b.player))
            .then(key_a.leader.cmp(&key_b.leader))
            .then(key_a.civilization.cmp(&key_b.civilization))
    });
    let mut out = String::from("Elo leaderboard (player × leader × civilization):\n");
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

#[cfg(test)]
mod tests {
    use super::{
        builtin_ai, builtin_provenance, collapsed_entrants, expected, league_generalist,
        scheduled_seats, seat_schedule, win_shares, EloPool, RatedPlayer, RatingKey, BUILTIN_AIS,
        CHAMPION_FILE, ELO_SCHEMA_VERSION, EVAL_ONLY_AIS, VALUENET_FILE,
    };
    use crate::rng::Rng;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

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
            const SCRIPTED: [&str; 15] = [
                "advanced",
                "advanced_blind_to_leaders",
                "advanced_counter_in_lane",
                "advanced_counter_stand_down",
                "advanced_settler_commit",
                "advanced_banking_dedication",
                "advanced_civ_blind",
                "advanced_food_first",
                "advanced_lane_reachable",
                "advanced_measured_dedication",
                "advanced_parallel_settlers",
                "advanced_relief_scoped",
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
        pool.save(&path).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"schema_version\": 2"));
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
                player("TechPriest", "Trajan", "Rome", 2, true),
                player("LabRat", "Cleopatra", "Egypt", 1, false),
            ),
            (
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
                    pool.record_game(&[results.0, results.1], 24.0)
                })
                .unwrap();
            })
        })
        .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let pool = EloPool::load(&path).unwrap();
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
