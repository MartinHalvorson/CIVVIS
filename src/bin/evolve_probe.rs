//! Measure the noise floor of the statistic `civvis evolve` actually ranks.
//!
//! The breeder selects normalized score plus combat-achievement share, and
//! every genome sees the same map seeds and candidate seats. This probe keeps
//! the historical outright-win bonus as a control. A Bernoulli standard error
//! describes neither the selected quantity nor the paired comparison available
//! to it.
//!
//! This probe evaluates the four bounded `Doctrine` perturbations through the
//! production fitness path, reports independent and common-seed paired
//! uncertainty, then treats consecutive blocks as separate eight-game
//! generations. If those blocks choose different leaders, the shipped budget
//! is selection drift on this measured effect even if the composite statistic
//! is quieter than win rate.
//!
//! ```text
//! cargo run --profile ci --bin evolve_probe -- \
//!   --games 64 --players 4 --turns 200 --seed 99000 --jobs 4
//! cargo run --profile ci --bin evolve_probe -- \
//!   --base evolved --games 64 --players 4 --turns 200 --seed 100000
//! ```
use std::collections::BTreeMap;

use civvis::ai::Weights;
use civvis::evolve::{fitness_observations, EvoCfg, FitnessObservation};
use civvis::parallel;
use civvis::strategic::Doctrine;

#[derive(Clone, Copy)]
enum Objective {
    Historical,
    NoWinBonus,
    ScoreOnly,
    ScoreWinBlend,
}

impl Objective {
    const ALL: [Objective; 4] = [
        Objective::Historical,
        Objective::NoWinBonus,
        Objective::ScoreOnly,
        Objective::ScoreWinBlend,
    ];

    fn name(self) -> &'static str {
        match self {
            Objective::Historical => "historical",
            Objective::NoWinBonus => "score+combat",
            Objective::ScoreOnly => "score-only",
            Objective::ScoreWinBlend => "80score+20win",
        }
    }

    fn value(self, observation: FitnessObservation, players: usize) -> f64 {
        match self {
            Objective::Historical => observation.value,
            Objective::NoWinBonus => {
                50.0 * players as f64 * observation.score_share
                    + 12.0 * players as f64 * observation.combat_share
            }
            Objective::ScoreOnly => 100.0 * observation.score_share,
            Objective::ScoreWinBlend => {
                80.0 * observation.score_share + 20.0 * f64::from(observation.won)
            }
        }
    }
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn mean_se(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    let values: Vec<f64> = values.into_iter().collect();
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, (variance / values.len() as f64).sqrt())
}

fn fitness_stats(
    observations: &[FitnessObservation],
    objective: Objective,
    players: usize,
) -> (f64, f64) {
    mean_se(
        observations
            .iter()
            .map(|observation| objective.value(*observation, players)),
    )
}

fn win_stats(observations: &[FitnessObservation]) -> (f64, f64) {
    mean_se(
        observations
            .iter()
            .map(|observation| f64::from(observation.won)),
    )
}

fn paired_stats(
    left: &[FitnessObservation],
    right: &[FitnessObservation],
    count: usize,
    objective: Objective,
    players: usize,
) -> (f64, f64) {
    mean_se(
        left.iter().zip(right).take(count).map(|(left, right)| {
            objective.value(*left, players) - objective.value(*right, players)
        }),
    )
}

fn ranked(
    results: &[Vec<FitnessObservation>],
    count: usize,
    objective: Objective,
    players: usize,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..results.len()).collect();
    order.sort_by(|left, right| {
        let left = fitness_stats(&results[*left][..count], objective, players).0;
        let right = fitness_stats(&results[*right][..count], objective, players).0;
        right
            .partial_cmp(&left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

fn block_winners<'a>(
    candidates: &'a [(&'a str, Weights)],
    results: &[Vec<FitnessObservation>],
    block: usize,
    objective: Objective,
    players: usize,
) -> BTreeMap<&'a str, usize> {
    let mut winners = BTreeMap::new();
    for chunk in 0..results[0].len() / block {
        let start = chunk * block;
        let end = start + block;
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by(|left, right| {
            let left = fitness_stats(&results[*left][start..end], objective, players).0;
            let right = fitness_stats(&results[*right][start..end], objective, players).0;
            right
                .partial_cmp(&left)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        *winners.entry(candidates[order[0]].0).or_default() += 1;
    }
    winners
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 64).max(1);
    let players = number(&args, "--players", 4).max(2);
    let turns = number(&args, "--turns", 200).max(1) as u32;
    let seed = number(&args, "--seed", 99_000) as u64;
    let jobs = number(&args, "--jobs", 4).max(1);
    let width = number(&args, "--width", 24).max(8) as i32;
    let height = number(&args, "--height", 16).max(8) as i32;
    let block = number(&args, "--block", 8).max(1);

    let (base, source) = match text(&args, "--base") {
        Some(dir) => match civvis::evolve::load_champion(&dir) {
            Some(weights) => (weights, format!("champion from {dir}")),
            None => {
                eprintln!("evolve_probe: no valid best.json in {dir:?}");
                std::process::exit(2);
            }
        },
        None => (Weights::default(), "Weights::default()".to_string()),
    };
    let candidates: Vec<(&'static str, Weights)> = Doctrine::ALL
        .into_iter()
        .map(|doctrine| (doctrine.name(), doctrine.apply(&base)))
        .collect();
    let opponents = vec![base.clone()];
    let cfg = EvoCfg {
        generations: 1,
        pop: candidates.len(),
        games,
        players,
        width,
        height,
        max_turns: turns,
        seed,
        threads: jobs,
        dir: String::new(),
    };

    let results = parallel::map(candidates.len(), jobs, |index| {
        fitness_observations(&candidates[index].1, &opponents, &cfg, 0, games)
    });
    let incumbent = candidates
        .iter()
        .position(|(name, _)| *name == "incumbent")
        .expect("Doctrine::ALL includes incumbent");

    println!(
        "evolve probe: {} candidates x {games} games on the exact training schedule",
        candidates.len()
    );
    println!(
        "base: {source}; {players}p {width}x{height}, {turns} turns (every third game doubles), seed {seed}"
    );
    println!();
    println!(
        "  {:<13} {:>9} {:>9} {:>10} {:>10} {:>10}",
        "candidate", "fitness", "SE", "win rate", "paired edge", "paired SE"
    );
    for (index, (name, _)) in candidates.iter().enumerate() {
        let (fitness, fitness_se) = fitness_stats(&results[index], Objective::NoWinBonus, players);
        let (wins, _) = win_stats(&results[index]);
        let (edge, edge_se) = paired_stats(
            &results[index],
            &results[incumbent],
            games,
            Objective::NoWinBonus,
            players,
        );
        println!(
            "  {name:<13} {fitness:>9.3} {fitness_se:>9.3} {wins:>9.1}% {edge:>10.3} {edge_se:>10.3}",
            wins = 100.0 * wins,
        );
    }

    let mut prefixes = vec![
        players.min(games),
        8.min(games),
        16.min(games),
        32.min(games),
        games,
    ];
    prefixes.sort_unstable();
    prefixes.dedup();
    println!();
    println!("prefix stability (the shipped default is K=8):");
    println!(
        "  {:>5}  {:<13} {:>10} {:>10}  {}",
        "games", "leader", "edge #2", "paired SE", "ranking"
    );
    for count in prefixes {
        let order = ranked(&results, count, Objective::NoWinBonus, players);
        let (edge, edge_se) = paired_stats(
            &results[order[0]],
            &results[order[1]],
            count,
            Objective::NoWinBonus,
            players,
        );
        let labels = order
            .iter()
            .map(|index| candidates[*index].0)
            .collect::<Vec<_>>()
            .join(" > ");
        println!(
            "  {count:>5}  {:<13} {edge:>10.3} {edge_se:>10.3}  {labels}",
            candidates[order[0]].0,
        );
    }

    let complete_blocks = games / block;
    let production_winners =
        block_winners(&candidates, &results, block, Objective::NoWinBonus, players);
    println!();
    println!("disjoint {block}-game selection blocks ({complete_blocks} complete):");
    for (name, count) in &production_winners {
        println!("  {name:<13} {count}");
    }

    println!();
    println!("objective comparison on the same simulated games:");
    println!(
        "  {:<17} {:<13} {:>10} {:>10} {:>12}",
        "objective", "full leader", "edge #2", "paired SE", "K-block wins"
    );
    for objective in Objective::ALL {
        let order = ranked(&results, games, objective, players);
        let (edge, edge_se) = paired_stats(
            &results[order[0]],
            &results[order[1]],
            games,
            objective,
            players,
        );
        let winners = block_winners(&candidates, &results, block, objective, players);
        let stable = winners.get(candidates[order[0]].0).copied().unwrap_or(0);
        println!(
            "  {:<17} {:<13} {edge:>10.3} {edge_se:>10.3} {:>5}/{:<5}",
            objective.name(),
            candidates[order[0]].0,
            stable,
            complete_blocks,
        );
    }

    let full_order = ranked(&results, games, Objective::NoWinBonus, players);
    let (full_edge, full_se) = paired_stats(
        &results[full_order[0]],
        &results[full_order[1]],
        games,
        Objective::NoWinBonus,
        players,
    );
    let full_winner_blocks = production_winners
        .get(candidates[full_order[0]].0)
        .copied()
        .unwrap_or(0);
    let stable_share = full_winner_blocks as f64 / complete_blocks.max(1) as f64;
    if full_edge > 2.0 * full_se && stable_share >= 0.75 {
        println!(
            "\nRESOLVED on this candidate set — the full leader clears two paired SE and \
             wins {:.0}% of disjoint K={block} selections.",
            100.0 * stable_share
        );
    } else {
        println!(
            "\nNOISE-DOMINATED at K={block} on this candidate set — full leader {} beats \
             the runner-up by {full_edge:.3} +/- {full_se:.3} and wins only {:.0}% of \
             disjoint selection blocks. Raising population size does not repair this; \
             selection needs more paired observations or a lower-variance objective.",
            candidates[full_order[0]].0,
            100.0 * stable_share,
        );
    }
}
