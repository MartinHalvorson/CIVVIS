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

use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi};
use crate::game::{Action, Game};
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::MapSize;

pub const BUILTIN_AIS: [&str; 9] = [
    "advanced",
    "advanced_evolved",
    "advanced_v1",
    "basic",
    "random",
    "evolved",
    "neural",
    "strategic",
    "policy",
];

/// Controls intended for paired evaluator experiments, not persistent
/// tournament ratings. Keeping them out of `BUILTIN_AIS` prevents a control
/// factory from being pooled into the same player/leader rating key as
/// its treatment.
pub const EVAL_ONLY_AIS: [&str; 1] = ["strategic_score"];

/// On-disk schema for the shared player/leader/civilization rating ledger.
pub const ELO_SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_RATINGS_PATH: &str = "data/elo_ratings.json";

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
        "advanced_evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_else(AdvancedAi::new),
        ),
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "random" => Box::new(RandomAi::new(seed)),
        "evolved" => Box::new(
            crate::evolve::load_champion("evolved")
                .map(AdvancedAi::with_weights)
                .unwrap_or_default(),
        ),
        "neural" => {
            let w = crate::evolve::load_champion("evolved").unwrap_or_default();
            match crate::valuenet::ValueNet::load("evolved") {
                Some(n) => Box::new(crate::neural::NeuralAi::new(w, n)),
                None => Box::new(BasicAi::with_weights(w)),
            }
        }
        "policy" => Box::new(crate::policy::PolicyAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        "strategic" => Box::new(crate::strategic::StrategicAi::with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        "strategic_score" => Box::new(crate::strategic::StrategicAi::score_only_with_weights(
            crate::evolve::load_champion("evolved").unwrap_or_default(),
        )),
        _ => Box::new(BasicAi::new()),
    }
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
        expected, scheduled_seats, seat_schedule, win_shares, EloPool, RatedPlayer, RatingKey,
        ELO_SCHEMA_VERSION,
    };
    use crate::rng::Rng;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

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
