//! Does the objective `evolve` breeds on rank seats the way winning does?
//!
//! `evolve` selects on a dense proxy and promotes on a sparse one:
//!
//! ```text
//! selection_value = 50 * players * score_share + 12 * players * combat_share
//! ```
//!
//! while `sprt_confirm` accepts a champion on wins alone. That split is
//! deliberate and it was measured — the historical scalar added 100 points for
//! one Bernoulli result to a 50-point score component, and on two disjoint
//! 64-game blocks that made the full leader win only 8/16 independent K=8
//! selections; removing the win bonus raised stability to 12/16 and halved the
//! paired standard error.
//!
//! **But that measured the proxy's *stability*, not its *alignment*.** A proxy
//! can be beautifully low-variance and point somewhere else, and then the search
//! operator climbs one hill while the acceptance test stands on another. The
//! league already shows the shape of that failure from a different angle: it
//! breeds and seats on mean placement while the gate promotes on wins, and an
//! agent has rated higher there while winning 3.5 times less.
//!
//! This measures alignment directly. Play whole games with the stock fleet and,
//! for each one, ask which seat the proxy would have picked and whether that
//! seat is the seat that won. Three numbers come out, all against the same
//! chance baseline of `1 / players`:
//!
//! - **selection hit rate** — how often the `selection_value` leader is the
//!   winner. This is the number that matters: it is the question the GA asks
//!   every generation.
//! - **score-only hit rate** — the same with the combat term removed, which says
//!   whether the 12-point combat share is helping the proxy or dragging it.
//! - **rank agreement** — mean Spearman between the proxy's ordering of the
//!   table and the ordering by final score, which catches a proxy that gets the
//!   winner right and the rest of the field wrong.
//!
//! ```text
//! proxy_align --games 60 --players 4 --turns 200
//! ```
//!
//! **What each outcome would mean.** Near `1 / players`, the proxy is noise and
//! the GA's whole search has been climbing something unrelated to its gate — a
//! structural finding that would explain a thousand rounds of evolution
//! producing no measurable gain. Well above it, the split is sound engineering
//! and the objection should be withdrawn. Anything in between is a real but
//! lossy proxy, and the honest response is to report the loss rather than to
//! call the design broken.
use civvis::ai::{run_game, AdvancedAi};
use civvis::game::Game;
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One game, reduced to what the question needs.
struct Table {
    /// Per-seat `selection_value`, in seat order.
    proxy: Vec<f64>,
    /// Per-seat proxy with the combat term dropped.
    score_only: Vec<f64>,
    /// Per-seat final score, the thing the proxy is built out of.
    scores: Vec<f64>,
    /// Which seat won, if the game was decided.
    winner: Option<usize>,
}

fn play(seed: u64, seats: usize, width: i32, height: i32, turns: u32) -> Table {
    let mut game = Game::new(seats, width, height, seed, turns, 0);
    let mut fleet = AdvancedAi::fleet(&game);
    run_game(&mut game, &mut fleet);

    // Rebuilt exactly as `evolve::eval_game_observation` does it, majors only,
    // so this measures the shipped objective and not a paraphrase of it.
    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| !player.is_minor)
        .map(|player| player.id)
        .collect();
    let scores: Vec<f64> = majors.iter().map(|pid| game.score(*pid) as f64).collect();
    let total: f64 = scores.iter().sum();
    let achievements: Vec<f64> = majors
        .iter()
        .map(|pid| {
            let player = &game.players[*pid];
            let kills = player.counters.get("kills").copied().unwrap_or(0) as f64;
            let captures = player.counters.get("captures").copied().unwrap_or(0) as f64;
            kills + captures * 3.0
        })
        .collect();
    let combat_total: f64 = achievements.iter().sum();
    let players = majors.len() as f64;

    let mut proxy = Vec::with_capacity(majors.len());
    let mut score_only = Vec::with_capacity(majors.len());
    for index in 0..majors.len() {
        let score_share = if total > 0.0 { scores[index] / total } else { 0.0 };
        let combat_share = if combat_total > 0.0 {
            achievements[index] / combat_total
        } else {
            0.0
        };
        proxy.push(50.0 * players * score_share + 12.0 * players * combat_share);
        score_only.push(50.0 * players * score_share);
    }
    let winner = game
        .winner
        .and_then(|pid| majors.iter().position(|major| *major == pid));
    Table {
        proxy,
        score_only,
        scores,
        winner,
    }
}

/// Index of the largest value, with ties going to the first — the same way a
/// selection that scanned for a maximum would resolve them.
fn leader(values: &[f64]) -> usize {
    let mut best = 0;
    for (index, value) in values.iter().enumerate() {
        if *value > values[best] {
            best = index;
        }
    }
    let _ = values;
    best
}

/// Spearman rank correlation between two orderings of the same table.
fn spearman(left: &[f64], right: &[f64]) -> f64 {
    let rank = |values: &[f64]| -> Vec<f64> {
        let mut order: Vec<usize> = (0..values.len()).collect();
        order.sort_by(|a, b| values[*a].partial_cmp(&values[*b]).unwrap());
        let mut ranks = vec![0.0; values.len()];
        for (place, index) in order.iter().enumerate() {
            ranks[*index] = place as f64;
        }
        ranks
    };
    let (a, b) = (rank(left), rank(right));
    let n = a.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mean = (n - 1.0) / 2.0;
    let (mut top, mut la, mut lb) = (0.0, 0.0, 0.0);
    for index in 0..a.len() {
        let (da, db) = (a[index] - mean, b[index] - mean);
        top += da * db;
        la += da * da;
        lb += db * db;
    }
    if la <= 0.0 || lb <= 0.0 {
        0.0
    } else {
        top / (la.sqrt() * lb.sqrt())
    }
}

/// Wilson 95% interval, so a hit rate arrives with the width it deserves.
fn wilson(hits: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (hits, n) = (hits as f64, n as f64);
    let p = hits / n;
    let z = 1.959_964;
    let denominator = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denominator;
    let spread = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((centre - spread).max(0.0), (centre + spread).min(1.0))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 60);
    let seats = number(&args, "--players", 4);
    let width = number(&args, "--width", 44) as i32;
    let height = number(&args, "--height", 28) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let seed = number(&args, "--seed", 95_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!("proxy_align: {games} games, {seats} players, {width}x{height}, {turns} turns");
    println!("objective under test: 50*P*score_share + 12*P*combat_share (evolve::selection_value)");

    let tables = parallel::map(games, jobs, |index| {
        play(seed + index as u64, seats, width, height, turns)
    });

    let decided: Vec<&Table> = tables.iter().filter(|t| t.winner.is_some()).collect();
    let mut proxy_hits = 0usize;
    let mut score_hits = 0usize;
    let mut rank_sum = 0.0;
    // The two objectives are read on the *same* games, so the comparison
    // between them is paired and the discordant pairs are what carries the
    // evidence. Comparing two unpaired proportions here would throw away the
    // pairing and widen the interval for nothing.
    let (mut only_score_right, mut only_proxy_right) = (0usize, 0usize);
    for table in &decided {
        let winner = table.winner.unwrap();
        let proxy_right = leader(&table.proxy) == winner;
        let score_right = leader(&table.score_only) == winner;
        proxy_hits += proxy_right as usize;
        score_hits += score_right as usize;
        only_score_right += (score_right && !proxy_right) as usize;
        only_proxy_right += (proxy_right && !score_right) as usize;
        rank_sum += spearman(&table.proxy, &table.scores);
    }
    let n = decided.len();
    let chance = 1.0 / seats as f64;

    println!(
        "\n{n}/{games} games decided by victory; chance baseline {:.1}%",
        100.0 * chance
    );
    let (low, high) = wilson(proxy_hits, n);
    println!(
        "selection_value leader is the winner: {proxy_hits}/{n} ({:.1}%, 95% CI {:.1}-{:.1}%)",
        100.0 * proxy_hits as f64 / n.max(1) as f64,
        100.0 * low,
        100.0 * high
    );
    let (slow, shigh) = wilson(score_hits, n);
    println!(
        "score-share alone leader is the winner: {score_hits}/{n} ({:.1}%, 95% CI {:.1}-{:.1}%)",
        100.0 * score_hits as f64 / n.max(1) as f64,
        100.0 * slow,
        100.0 * shigh
    );
    println!(
        "mean Spearman(proxy ordering, score ordering): {:.3}",
        rank_sum / n.max(1) as f64
    );
    // Exact two-sided sign test on the discordant pairs -- McNemar without the
    // chi-square approximation, which is wrong at these counts.
    let discordant = only_score_right + only_proxy_right;
    let smaller = only_score_right.min(only_proxy_right);
    let mut tail = 0.0f64;
    for k in 0..=smaller {
        let mut term = 0.5f64.powi(discordant as i32);
        for i in 0..k {
            term *= (discordant - i) as f64 / (i + 1) as f64;
        }
        tail += term;
    }
    let p = (2.0 * tail).min(1.0);
    println!(
        "\ncombat term: it alone is right in {only_proxy_right} games, score alone in \
         {only_score_right}; {discordant} discordant pairs, exact two-sided p = {p:.4}"
    );
    // The verdict is stated rather than left to the reader, because the whole
    // point is to settle whether the objection to the split survives.
    let rate = proxy_hits as f64 / n.max(1) as f64;
    println!(
        "\nverdict: the proxy is {}",
        if low > chance * 1.5 {
            "aligned with winning well above chance -- the split is sound and the objection does not stand"
        } else if high < chance * 1.5 {
            "close to chance -- the GA has been climbing a hill its own gate does not stand on"
        } else {
            "inconclusive at this sample size -- the interval spans the threshold, so run more games"
        }
    );
}
