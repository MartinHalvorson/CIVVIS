//! Is there a cheap fitness that predicts winning?
//!
//! A genetic search over this genome is trapped between two fitnesses and
//! neither works. A **win rate** is the thing we care about and costs about
//! 865 games per genome to resolve a 0.05 effect. **Terminal score share** is
//! affordable at roughly a fifth of that variance and has now been shown not
//! to convert: `settler_min_pop = 5` gained +0.0187 ± 0.0062 of score share
//! across four disjoint seeds and returned 12 map directions to 15 on wins,
//! p=0.7011.
//!
//! That is the binding constraint on breeding here, and it is worth one
//! experiment before accepting it. **If some other cheap end-of-game quantity
//! predicts victory much better than score does, a GA becomes viable again**
//! — selection could read that instead, and only the final promotion would
//! need games.
//!
//! So: play games, and for every major seat record several candidate proxies
//! next to whether that seat actually won. Then score each proxy by how much
//! it tells you about winning.
//!
//! The measure is **AUC** — the chance a randomly chosen winner outranks a
//! randomly chosen non-winner on that proxy. 0.5 is a coin flip and 1.0 is
//! perfect. It is used instead of a correlation because it is invariant to any
//! monotone rescaling, so a proxy is not penalised for living on a different
//! scale, and because it is exactly the question selection asks: *given two
//! genomes, does this quantity rank them the way winning would?*
//!
//! ```text
//! proxy_fit --maps 60 --players 4
//! ```
//!
//! ## The answer, and the reading it required
//!
//! Over 60 games, 240 seat-games, every one decided:
//!
//! | proxy | AUC |
//! |---|---|
//! | **score share** | **0.949** |
//! | civic share | 0.890 |
//! | population share | 0.887 |
//! | tech share | 0.860 |
//! | city share | 0.854 |
//! | military power share | 0.755 |
//! | gold share | 0.714 |
//! | faith share | 0.611 |
//!
//! Score share is a **near-perfect classifier of the winner** and the best of
//! the eight. It is not a poor proxy — it is an excellent one.
//!
//! Which has to be squared with the measured fact that raising mean score share
//! by +0.019 across four seeds returned 12 map directions to 15 on wins. Both
//! are true, and together they locate the defect precisely: within a game the
//! winner is almost always the top scorer, **and** a couple of points of *mean*
//! share almost never flips who is first — it pads games already won or already
//! lost.
//!
//! **So the defect is the functional, not the signal.** Winning is a threshold
//! on score (be first), and the mean of a threshold quantity is a bad fitness
//! for it. A selection statistic here should be **convex** in score share,
//! rewarding being at the top far more than being two points above average,
//! which keeps most of the 0.949 discrimination while using more of each game
//! than a binary win does.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::rng::Rng;

/// Everything recorded for one major seat at the end of one game.
struct Seat {
    won: bool,
    proxies: Vec<f64>,
}

const PROXY_NAMES: [&str; 8] = [
    "score share",
    "city share",
    "tech share",
    "civic share",
    "military power share",
    "population share",
    "gold share",
    "faith share",
];

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Area under the ROC curve of `values` against `labels`, by rank sum.
///
/// Ties are handled by mid-rank, which matters here: several proxies saturate
/// or sit at zero for whole games, and counting a tie as a win would flatter
/// exactly the proxies that discriminate least.
fn auc(samples: &[(f64, bool)]) -> f64 {
    let positives = samples.iter().filter(|(_, won)| *won).count() as f64;
    let negatives = samples.len() as f64 - positives;
    if positives == 0.0 || negatives == 0.0 {
        return f64::NAN;
    }
    let mut order: Vec<usize> = (0..samples.len()).collect();
    order.sort_by(|a, b| {
        samples[*a]
            .0
            .partial_cmp(&samples[*b].0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0f64; samples.len()];
    let mut index = 0;
    while index < order.len() {
        let mut end = index;
        while end + 1 < order.len()
            && (samples[order[end + 1]].0 - samples[order[index]].0).abs() < 1e-12
        {
            end += 1;
        }
        let mid = (index + end) as f64 / 2.0 + 1.0;
        for slot in &order[index..=end] {
            ranks[*slot] = mid;
        }
        index = end + 1;
    }
    let positive_rank_sum: f64 = samples
        .iter()
        .enumerate()
        .filter(|(_, (_, won))| *won)
        .map(|(i, _)| ranks[i])
        .sum();
    (positive_rank_sum - positives * (positives + 1.0) / 2.0) / (positives * negatives)
}

fn share(values: &[f64], index: usize) -> f64 {
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    values[index] / total
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 60);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 1_800_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "proxy_fit: {maps} games, {players}p {width}x{height}, {turns} turns, seed {seed0}"
    );
    println!("  every seat carries a genome drawn at random, so the proxies see real spread");
    println!("  AUC = P(a winner outranks a non-winner); 0.500 is a coin flip\n");

    let bounds = Weights::bounds();
    let seats: Vec<Vec<Seat>> = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        // Random genomes per seat. A table of identical agents would make every
        // proxy look uninformative for the trivial reason that the seats are
        // interchangeable; selection operates on genomes that differ, so the
        // measurement has to as well.
        let mut rng = Rng::new(seed ^ 0xA5A5_5A5A_1234_5678);
        let mut fleet: Vec<AdvancedAi> = Vec::new();
        for _ in 0..game.players.len() {
            let mut v = Weights::default().to_vec();
            for (gene, (lo, hi)) in v.iter_mut().zip(bounds) {
                if rng.chance(0.35) {
                    *gene = rng.uniform(lo, hi);
                }
            }
            fleet.push(AdvancedAi::with_weights(Weights::from_vec(&v)));
        }
        for _ in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor)
            .collect();
        let column = |f: &dyn Fn(usize) -> f64| -> Vec<f64> {
            majors.iter().map(|pid| f(*pid)).collect()
        };
        let scores = column(&|pid| game.score(pid) as f64);
        let cities = column(&|pid| game.player_city_ids(pid).len() as f64);
        let techs = column(&|pid| game.players[pid].techs.len() as f64);
        let civics = column(&|pid| game.players[pid].civics.len() as f64);
        let power = column(&|pid| game.military_power(pid));
        let pop = column(&|pid| {
            game.player_city_ids(pid)
                .iter()
                .filter_map(|cid| game.cities.get(cid))
                .map(|city| city.pop as f64)
                .sum()
        });
        let gold = column(&|pid| game.players[pid].gold.max(0.0));
        let faith = column(&|pid| game.players[pid].faith.max(0.0));

        majors
            .iter()
            .enumerate()
            .map(|(slot, pid)| Seat {
                won: game.winner == Some(*pid),
                proxies: vec![
                    share(&scores, slot),
                    share(&cities, slot),
                    share(&techs, slot),
                    share(&civics, slot),
                    share(&power, slot),
                    share(&pop, slot),
                    share(&gold, slot),
                    share(&faith, slot),
                ],
            })
            .collect()
    });

    let flat: Vec<Seat> = seats.into_iter().flatten().collect();
    let winners = flat.iter().filter(|s| s.won).count();
    println!(
        "  {} seat-games, {winners} with a victory ({:.0}% of games decided)\n",
        flat.len(),
        100.0 * winners as f64 * players as f64 / flat.len() as f64
    );

    let mut rows: Vec<(f64, &str)> = Vec::new();
    for (index, name) in PROXY_NAMES.iter().enumerate() {
        let samples: Vec<(f64, bool)> = flat
            .iter()
            .map(|seat| (seat.proxies[index], seat.won))
            .collect();
        let value = auc(&samples);
        rows.push((value, name));
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (value, name) in &rows {
        println!("  {name:<22} AUC {value:.3}");
    }

    let best = rows[0];
    let score_auc = rows
        .iter()
        .find(|(_, name)| *name == "score share")
        .map(|(v, _)| *v)
        .unwrap_or(f64::NAN);
    println!("\nbest proxy: {} at AUC {:.3}; score share is {score_auc:.3}", best.1, best.0);

    // The first version of this conclusion was wrong, and the error is worth
    // keeping visible because it is easy to repeat: it compared the best proxy
    // against score share, found no separation, and reported that as "no cheap
    // quantity predicts winning". But score share came in at AUC 0.949 -- it is
    // very nearly a perfect classifier of who won, and the best of the eight.
    // "Nothing beats the best" is not "the best is bad".
    if score_auc > 0.85 {
        println!(
            "  => score share is a NEAR-PERFECT classifier of the winner, not a poor one.\n\
             \n     \
             That has to be reconciled with the measured fact that raising mean score share\n     \
             by +0.019 across four seeds produced 12 map directions to 15 on wins. Both are\n     \
             true: within a game the winner is almost always the top scorer, AND a couple of\n     \
             points of MEAN share almost never flips who is first -- it pads games already\n     \
             won or already lost.\n\
             \n     \
             So the defect is not the signal, it is the functional. Winning is a THRESHOLD on\n     \
             score (be first), and the mean of a threshold quantity is a bad fitness for it.\n     \
             A selection statistic here should be convex in score share -- rewarding being at\n     \
             the top far more than being two points above average -- which keeps most of the\n     \
             0.949 discrimination while using more of each game than a binary win does."
        );
    } else if best.0 - score_auc > 0.05 {
        println!(
            "  => {} ranks seats by victory materially better than score does. A selection\n     \
             fitness built on it would be both cheap and closer to the thing that matters.\n     \
             Worth confirming that a genome bred on it beats one bred on score, ON WINS.",
            best.1
        );
    } else {
        println!(
            "  => no proxy discriminates well and none beats the others. Selection would have\n     \
             to buy wins directly."
        );
    }
}
