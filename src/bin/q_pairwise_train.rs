//! Fit a replica-aware move ranker to counterfactual continuation outcomes.
//!
//! `q_counterfactual` evaluates every candidate under matched opponent
//! doctrines. Averaging those returns before training hides whether a move is
//! consistently better or merely wins under one continuation. This trainer
//! keeps the replica vector, turns every candidate pair into a
//! Jeffreys-smoothed probability of superiority, and fits a linear logistic
//! utility to feature differences.
//!
//! At evaluation time the recorded expert remains the default. The model may
//! replace it only when the predicted probability that the best alternative
//! beats the expert clears `--override-probability` (0.70 by default).
//!
//! ```text
//! q_pairwise_train --data /tmp/q-standard.csv \
//!   --eval-data /tmp/q-online.csv --keep destination \
//!   --out /tmp/q-pairwise.json
//! ```
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};

const EPS: f32 = 1e-6;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn decimal(args: &[String], flag: &str, default: f32) -> f32 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

#[derive(Clone)]
struct Group {
    rows: Vec<Vec<f32>>,
    returns: Vec<Vec<f32>>,
    means: Vec<f32>,
    game: u64,
    decision: (u32, usize, u32), // turn, seat, unit
}

struct Loaded {
    groups: Vec<Group>,
    width: usize,
    replicas: usize,
    rows: usize,
}

/// Keep feature-block definitions identical to `q_train` and
/// `q_advantage_train`, so changing the target does not also change what the
/// model can see.
fn mask(row: &mut [f32], keep: &str) {
    let width = row.len();
    let state = width - civvis::action_space::FEATURE_WIDTH;
    let kinds = civvis::action_space::KINDS.len();
    let legacy = state + kinds;
    let destination = legacy + civvis::action_space::LEGACY_NUMERIC_WIDTH;
    let plan = destination + civvis::action_space::PLAN_OFFSET;
    let plan_end = plan + civvis::action_space::PLAN_WIDTH;
    let blank = |row: &mut [f32], from: usize, to: usize| {
        for value in row.iter_mut().take(to.min(width)).skip(from) {
            *value = 0.0;
        }
    };
    match keep {
        "state" => blank(row, state, width),
        "action" => blank(row, 0, state),
        "kind" => {
            blank(row, 0, state);
            blank(row, state + kinds, width);
        }
        "geometry" => blank(row, 0, state + kinds),
        "legacy-geometry" => {
            blank(row, 0, legacy);
            blank(row, destination, width);
        }
        "destination" => blank(row, 0, destination),
        "destination-no-plan" => {
            blank(row, 0, destination);
            blank(row, plan, plan_end);
        }
        "plan" => {
            blank(row, 0, plan);
            blank(row, plan_end, width);
        }
        "all" => {}
        _ => panic!("unsupported feature block {keep}"),
    }
}

fn finish_group(group: Group, path: &str) -> Result<Group, String> {
    if group.rows.len() < 2
        || group.rows.len() != group.returns.len()
        || group.rows.len() != group.means.len()
    {
        return Err(format!(
            "{path}: game {} decision {:?} has incomplete candidates",
            group.game, group.decision
        ));
    }
    let replicas = group.returns[0].len();
    if replicas == 0 || group.returns.iter().any(|values| values.len() != replicas) {
        return Err(format!(
            "{path}: game {} decision {:?} has inconsistent replicas",
            group.game, group.decision
        ));
    }
    Ok(group)
}

fn parse_value(field: &str, path: &str, line: usize, name: &str) -> Result<f32, String> {
    field
        .parse::<f32>()
        .map_err(|error| format!("{path}:{line}: invalid {name} {field:?}: {error}"))
}

fn load_groups(path: &str, keep: &str) -> Result<Loaded, String> {
    let file = fs::File::open(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|error| format!("cannot read header from {path}: {error}"))?;
    let names: Vec<&str> = header.trim_end().split(',').collect();
    let return_column = names
        .iter()
        .position(|name| *name == "return")
        .ok_or_else(|| format!("{path}: no counterfactual return column"))?;
    if return_column <= 5 {
        return Err(format!("{path}: return column precedes candidate features"));
    }
    let raw_width = return_column - 5; // game, turn, seat, unit, chosen
    let expected = civvis::decision_features::WIDTH + civvis::action_space::FEATURE_WIDTH;
    if raw_width != expected {
        return Err(format!(
            "{path}: {raw_width} candidate features do not match current schema {expected}"
        ));
    }
    let replicas = names.len().saturating_sub(return_column + 1);
    if replicas == 0 {
        return Err(format!("{path}: no replica columns after return"));
    }
    for replica in 0..replicas {
        let expected = format!("r{replica}");
        if names[return_column + 1 + replica] != expected {
            return Err(format!(
                "{path}: expected replica column {expected}, found {}",
                names[return_column + 1 + replica]
            ));
        }
    }

    let mut groups = Vec::new();
    let mut current: Option<Group> = None;
    let mut rows = 0usize;
    for (offset, line) in reader.lines().enumerate() {
        let line_number = offset + 2;
        let line = line.map_err(|error| format!("{path}:{line_number}: {error}"))?;
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != names.len() {
            return Err(format!(
                "{path}:{line_number}: {} fields, expected {}",
                fields.len(),
                names.len()
            ));
        }
        rows += 1;
        let game = fields[0]
            .parse::<u64>()
            .map_err(|error| format!("{path}:{line_number}: invalid game: {error}"))?;
        let decision = (
            fields[1]
                .parse::<u32>()
                .map_err(|error| format!("{path}:{line_number}: invalid turn: {error}"))?,
            fields[2]
                .parse::<usize>()
                .map_err(|error| format!("{path}:{line_number}: invalid seat: {error}"))?,
            fields[3]
                .parse::<u32>()
                .map_err(|error| format!("{path}:{line_number}: invalid unit: {error}"))?,
        );
        let chosen = match fields[4] {
            "1" => true,
            "0" => false,
            value => {
                return Err(format!(
                    "{path}:{line_number}: invalid chosen flag {value:?}"
                ))
            }
        };
        let mut candidate = fields[5..return_column]
            .iter()
            .enumerate()
            .map(|(index, field)| {
                parse_value(field, path, line_number, &format!("feature {index}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        mask(&mut candidate, keep);
        let mean = parse_value(fields[return_column], path, line_number, "return")?;
        let replica_returns = fields[return_column + 1..]
            .iter()
            .enumerate()
            .map(|(index, field)| {
                parse_value(field, path, line_number, &format!("replica {index}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if candidate.iter().any(|value| !value.is_finite())
            || !mean.is_finite()
            || replica_returns.iter().any(|value| !value.is_finite())
        {
            return Err(format!(
                "{path}:{line_number}: candidate contains a non-finite value"
            ));
        }
        let replica_mean =
            replica_returns.iter().sum::<f32>() / replica_returns.len().max(1) as f32;
        if (mean - replica_mean).abs() > 2e-6 {
            return Err(format!(
                "{path}:{line_number}: return {mean} does not match replica mean {replica_mean}"
            ));
        }

        if chosen {
            if let Some(group) = current.take() {
                groups.push(finish_group(group, path)?);
            }
            current = Some(Group {
                rows: vec![candidate],
                returns: vec![replica_returns],
                means: vec![mean],
                game,
                decision,
            });
        } else if let Some(group) = current.as_mut() {
            if group.game != game || group.decision != decision {
                return Err(format!(
                    "{path}:{line_number}: alternative does not follow its chosen row"
                ));
            }
            group.rows.push(candidate);
            group.returns.push(replica_returns);
            group.means.push(mean);
        } else {
            return Err(format!(
                "{path}:{line_number}: alternative appears before a chosen row"
            ));
        }
    }
    if let Some(group) = current.take() {
        groups.push(finish_group(group, path)?);
    }
    if groups.is_empty() {
        return Err(format!("{path}: no complete decisions"));
    }
    Ok(Loaded {
        groups,
        width: raw_width,
        replicas,
        rows,
    })
}

fn holdout(game: u64, share: f32) -> bool {
    let mut hash = game.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^= hash >> 32;
    (hash % 1000) as f32 / 1000.0 < share
}

struct Linear {
    weights: Vec<f32>,
}

impl Linear {
    fn new(width: usize) -> Linear {
        Linear {
            weights: vec![0.0; width],
        }
    }

    fn score(&self, row: &[f32]) -> f32 {
        self.weights
            .iter()
            .zip(row)
            .map(|(weight, value)| weight * value)
            .sum()
    }
}

fn sigmoid(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

/// Jeffreys-smoothed posterior mean for `P(left > right)`. Exact return ties
/// split their vote. Four replicas therefore map 4-0, 3-1, and 2-2 evidence to
/// 0.90, 0.70, and 0.50 rather than pretending four observations are certain.
fn superiority_target(left: &[f32], right: &[f32]) -> f32 {
    assert_eq!(left.len(), right.len());
    let successes = left.iter().zip(right).fold(0.0, |sum, (left, right)| {
        sum + if left - right > EPS {
            1.0
        } else if right - left > EPS {
            0.0
        } else {
            0.5
        }
    });
    (successes + 0.5) / (left.len() as f32 + 1.0)
}

fn logistic_loss(logit: f32, target: f32) -> f32 {
    if logit >= 0.0 {
        (1.0 - target) * logit + (-logit).exp().ln_1p()
    } else {
        -target * logit + logit.exp().ln_1p()
    }
}

/// Every unordered candidate pair contributes equally. A split doctrine vote
/// has target 0.5 and pulls an unsupported margin back toward zero.
fn group_loss(net: &Linear, group: &Group, grad: Option<&mut [f32]>) -> (f32, usize) {
    let scores: Vec<f32> = group.rows.iter().map(|row| net.score(row)).collect();
    let mut loss = 0.0;
    let mut pairs = 0usize;
    let mut grad = grad;
    for left in 0..group.rows.len() {
        for right in left + 1..group.rows.len() {
            let target = superiority_target(&group.returns[left], &group.returns[right]);
            let logit = scores[left] - scores[right];
            loss += logistic_loss(logit, target);
            pairs += 1;
            if let Some(gradient) = grad.as_deref_mut() {
                let seed = sigmoid(logit) - target;
                for ((value, left_value), right_value) in gradient
                    .iter_mut()
                    .zip(&group.rows[left])
                    .zip(&group.rows[right])
                {
                    *value += seed * (left_value - right_value);
                }
            }
        }
    }
    (loss, pairs)
}

#[derive(Default)]
struct GameMetrics {
    decisions: usize,
    spread: f32,
    chance_regret: f32,
    expert_regret: f32,
    ungated_regret: f32,
    gated_regret: f32,
    ungated_lift: f32,
    gated_lift: f32,
    overrides: usize,
    positive_overrides: usize,
    tied_overrides: usize,
    negative_overrides: usize,
    doctrine_wins: usize,
    doctrine_ties: usize,
    doctrine_losses: usize,
}

fn best_score(scores: &[f32]) -> usize {
    scores
        .iter()
        .enumerate()
        .skip(1)
        .fold(0, |best, (index, score)| {
            if *score > scores[best] + EPS {
                index
            } else {
                best
            }
        })
}

fn gated_choice(scores: &[f32], threshold: f32) -> usize {
    if scores.len() < 2 {
        return 0;
    }
    let alternative = scores
        .iter()
        .enumerate()
        .skip(1)
        .fold(1, |best, (index, score)| {
            if *score > scores[best] + EPS {
                index
            } else {
                best
            }
        });
    (sigmoid(scores[alternative] - scores[0]) + EPS >= threshold)
        .then_some(alternative)
        .unwrap_or(0)
}

fn evaluate(net: &Linear, groups: &[Group], threshold: f32) -> BTreeMap<u64, GameMetrics> {
    let mut games = BTreeMap::<u64, GameMetrics>::new();
    for group in groups {
        let scores: Vec<f32> = group.rows.iter().map(|row| net.score(row)).collect();
        let ungated = best_score(&scores);
        let gated = gated_choice(&scores, threshold);
        let best = group
            .means
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let low = group.means.iter().copied().fold(f32::INFINITY, f32::min);
        let chance = group.means.iter().sum::<f32>() / group.means.len() as f32;
        let expert = group.means[0];
        let metrics = games.entry(group.game).or_default();
        metrics.decisions += 1;
        metrics.spread += best - low;
        metrics.chance_regret += best - chance;
        metrics.expert_regret += best - expert;
        metrics.ungated_regret += best - group.means[ungated];
        metrics.gated_regret += best - group.means[gated];
        metrics.ungated_lift += group.means[ungated] - expert;
        metrics.gated_lift += group.means[gated] - expert;
        if gated != 0 {
            metrics.overrides += 1;
            let difference = group.means[gated] - expert;
            if difference > EPS {
                metrics.positive_overrides += 1;
            } else if difference < -EPS {
                metrics.negative_overrides += 1;
            } else {
                metrics.tied_overrides += 1;
            }
            for (candidate, expert) in group.returns[gated].iter().zip(&group.returns[0]) {
                if candidate - expert > EPS {
                    metrics.doctrine_wins += 1;
                } else if expert - candidate > EPS {
                    metrics.doctrine_losses += 1;
                } else {
                    metrics.doctrine_ties += 1;
                }
            }
        }
    }
    games
}

fn mean_se(values: &[f32]) -> (f32, f32) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (*value - mean).powi(2))
        .sum::<f32>()
        / (values.len() - 1) as f32;
    (mean, (variance / values.len() as f32).sqrt())
}

fn metric(games: &BTreeMap<u64, GameMetrics>, read: impl Fn(&GameMetrics) -> f32) -> Vec<f32> {
    games
        .values()
        .map(|game| read(game) / game.decisions.max(1) as f32)
        .collect()
}

fn percentile(sorted: &[f32], quantile: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f32 * quantile.clamp(0.0, 1.0)).round() as usize;
    sorted[index]
}

fn confidence_distribution(net: &Linear, groups: &[Group]) -> Vec<f32> {
    let mut probabilities = Vec::with_capacity(groups.len());
    for group in groups {
        let scores: Vec<f32> = group.rows.iter().map(|row| net.score(row)).collect();
        let alternative = scores
            .iter()
            .enumerate()
            .skip(1)
            .fold(1, |best, (index, score)| {
                if *score > scores[best] + EPS {
                    index
                } else {
                    best
                }
            });
        probabilities.push(sigmoid(scores[alternative] - scores[0]));
    }
    probabilities.sort_by(f32::total_cmp);
    probabilities
}

fn target_census(groups: &[Group], label: &str) {
    let mut targets = [0usize; 10];
    for group in groups {
        for left in 0..group.rows.len() {
            for right in left + 1..group.rows.len() {
                let target = superiority_target(&group.returns[left], &group.returns[right]);
                targets[(target * 10.0).round().clamp(0.0, 9.0) as usize] += 1;
            }
        }
    }
    let total: usize = targets.iter().sum();
    let populated = targets
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(tenths, count)| format!("{:.1}:{count}", tenths as f32 / 10.0))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{label} pair posterior targets ({total} pairs): {populated}");
}

struct Report {
    lift: f32,
    lift_se: f32,
    override_rate: f32,
}

fn report(net: &Linear, groups: &[Group], threshold: f32, label: &str) -> Report {
    let games = evaluate(net, groups, threshold);
    let decisions: usize = games.values().map(|game| game.decisions).sum();
    let overrides: usize = games.values().map(|game| game.overrides).sum();
    let positive: usize = games.values().map(|game| game.positive_overrides).sum();
    let tied: usize = games.values().map(|game| game.tied_overrides).sum();
    let negative: usize = games.values().map(|game| game.negative_overrides).sum();
    let doctrine_wins: usize = games.values().map(|game| game.doctrine_wins).sum();
    let doctrine_ties: usize = games.values().map(|game| game.doctrine_ties).sum();
    let doctrine_losses: usize = games.values().map(|game| game.doctrine_losses).sum();
    let (spread, _) = mean_se(&metric(&games, |game| game.spread));
    let (chance_regret, _) = mean_se(&metric(&games, |game| game.chance_regret));
    let (expert_regret, _) = mean_se(&metric(&games, |game| game.expert_regret));
    let (ungated_regret, _) = mean_se(&metric(&games, |game| game.ungated_regret));
    let (gated_regret, _) = mean_se(&metric(&games, |game| game.gated_regret));
    let (ungated_lift, ungated_lift_se) = mean_se(&metric(&games, |game| game.ungated_lift));
    let (lift, lift_se) = mean_se(&metric(&games, |game| game.gated_lift));
    let (override_rate, override_rate_se) = mean_se(&metric(&games, |game| game.overrides as f32));
    let confidence = confidence_distribution(net, groups);
    println!(
        "{label}: {decisions} decisions in {} games, mean return spread {spread:.4}",
        games.len()
    );
    println!(
        "  oracle regret: chance {chance_regret:.4}  expert {expert_regret:.4}  \
         ungated {ungated_regret:.4}  gated {gated_regret:.4}"
    );
    println!(
        "  return lift vs expert: ungated {ungated_lift:+.4} +/- {ungated_lift_se:.4}; \
         gated {lift:+.4} +/- {lift_se:.4} (95% lower {:+.4})",
        lift - 1.96 * lift_se
    );
    println!(
        "  gated overrides: {overrides}/{decisions} ({:.1}% game-macro +/- {:.1}%); \
         mean outcomes +/=/− {positive}/{tied}/{negative}",
        100.0 * override_rate,
        100.0 * override_rate_se
    );
    println!(
        "  best-sibling P(beat expert): p50 {:.3}, p90 {:.3}, p99 {:.3}, max {:.3}; \
         clear 0.55/0.60/0.65/{threshold:.2}: {}/{}/{}/{}",
        percentile(&confidence, 0.50),
        percentile(&confidence, 0.90),
        percentile(&confidence, 0.99),
        percentile(&confidence, 1.00),
        confidence
            .iter()
            .filter(|value| **value + EPS >= 0.55)
            .count(),
        confidence
            .iter()
            .filter(|value| **value + EPS >= 0.60)
            .count(),
        confidence
            .iter()
            .filter(|value| **value + EPS >= 0.65)
            .count(),
        confidence
            .iter()
            .filter(|value| **value + EPS >= threshold)
            .count()
    );
    println!(
        "  matched doctrine outcomes on overrides: +/=/− \
         {doctrine_wins}/{doctrine_ties}/{doctrine_losses}"
    );
    Report {
        lift,
        lift_se,
        override_rate,
    }
}

fn train(
    groups: &[Group],
    width: usize,
    epochs: usize,
    batch: usize,
    rate: f32,
    l2: f32,
) -> Linear {
    let mut net = Linear::new(width);
    let mut grad = vec![0.0f32; width];
    let mut order: Vec<usize> = (0..groups.len()).collect();
    let mut shuffle = 0x243F_6A88_85A3_08D3u64;
    for epoch in 1..=epochs {
        for index in (1..order.len()).rev() {
            shuffle = shuffle
                .wrapping_add(0x9E37_79B9_7F4A_7C15)
                .rotate_left(31)
                .wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let swap = (shuffle >> 33) as usize % (index + 1);
            order.swap(index, swap);
        }
        let mut loss = 0.0;
        let mut epoch_pairs = 0usize;
        let mut pending_groups = 0usize;
        let mut pending_pairs = 0usize;
        for (step, index) in order.iter().enumerate() {
            let (group_loss, pairs) = group_loss(&net, &groups[*index], Some(&mut grad));
            loss += group_loss;
            epoch_pairs += pairs;
            pending_pairs += pairs;
            pending_groups += 1;
            if pending_groups == batch || step + 1 == order.len() {
                let scale = rate / pending_pairs.max(1) as f32;
                for (weight, gradient) in net.weights.iter_mut().zip(&mut grad) {
                    *weight -= scale * (*gradient + l2 * *weight * pending_pairs as f32);
                    *gradient = 0.0;
                }
                pending_groups = 0;
                pending_pairs = 0;
            }
        }
        if epoch == 1 || epoch % 10 == 0 || epoch == epochs {
            println!(
                "epoch {epoch:>3}: train pairwise loss {:.4} over {epoch_pairs} pairs",
                loss / epoch_pairs.max(1) as f32
            );
        }
    }
    net
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = text(&args, "--data", "/tmp/q-counterfactual.csv");
    let eval_data = args
        .iter()
        .position(|arg| arg == "--eval-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let out = text(&args, "--out", "/tmp/q-pairwise.json");
    let keep = text(&args, "--keep", "destination");
    let epochs = number(&args, "--epochs", 80);
    let batch = number(&args, "--batch", 32).max(1);
    let rate = decimal(&args, "--rate", 0.05);
    let l2 = decimal(&args, "--l2", 0.0001).max(0.0);
    let holdout_share = decimal(&args, "--holdout", 0.25).clamp(0.0, 1.0);
    let threshold = decimal(&args, "--override-probability", 0.70).clamp(0.5, 1.0);
    if !matches!(
        keep.as_str(),
        "state"
            | "action"
            | "kind"
            | "geometry"
            | "legacy-geometry"
            | "destination"
            | "destination-no-plan"
            | "plan"
            | "all"
    ) {
        eprintln!("q_pairwise_train: unsupported --keep {keep}");
        std::process::exit(2);
    }
    let loaded = load_groups(&data, &keep).unwrap_or_else(|error| {
        eprintln!("q_pairwise_train: {error}");
        std::process::exit(2);
    });
    println!(
        "{data}: {} rows -> {} decisions, width {}, {} replicas, keep {keep}",
        loaded.rows,
        loaded.groups.len(),
        loaded.width,
        loaded.replicas
    );
    let width = loaded.width;
    let replicas = loaded.replicas;
    if replicas != 4 {
        eprintln!(
            "q_pairwise_train: preregistered experiment requires four replicas, found {replicas}"
        );
        std::process::exit(2);
    }
    let (train_groups, evaluation, label) = if let Some(path) = eval_data.as_deref() {
        let external = load_groups(path, &keep).unwrap_or_else(|error| {
            eprintln!("q_pairwise_train: {error}");
            std::process::exit(2);
        });
        if external.width != width || external.replicas != replicas {
            eprintln!("q_pairwise_train: train/evaluation schemas differ");
            std::process::exit(2);
        }
        println!(
            "external {path}: {} rows -> {} decisions, {} replicas",
            external.rows,
            external.groups.len(),
            external.replicas
        );
        (loaded.groups, external.groups, "external")
    } else {
        let mut train = Vec::new();
        let mut valid = Vec::new();
        for group in loaded.groups {
            if holdout(group.game, holdout_share) {
                valid.push(group);
            } else {
                train.push(group);
            }
        }
        (train, valid, "held-out")
    };
    if train_groups.is_empty() || evaluation.is_empty() {
        eprintln!("q_pairwise_train: need both training and evaluation games");
        std::process::exit(2);
    }
    println!(
        "{} train decisions, {} {label} decisions; fixed {epochs} epochs, override probability {threshold:.2}",
        train_groups.len(),
        evaluation.len()
    );
    target_census(&train_groups, "train");
    target_census(&evaluation, label);

    let net = train(&train_groups, width, epochs, batch, rate, l2);
    println!(
        "model scale: L2 norm {:.4}, largest |weight| {:.4}",
        net.weights
            .iter()
            .map(|weight| weight * weight)
            .sum::<f32>()
            .sqrt(),
        net.weights
            .iter()
            .map(|weight| weight.abs())
            .fold(0.0, f32::max)
    );
    report(&net, &train_groups, threshold, "train");
    let result = report(&net, &evaluation, threshold, label);
    let pass = if label == "external" {
        result.lift - 1.96 * result.lift_se > 0.0 && result.override_rate >= 0.05
    } else {
        result.lift > 0.0 && result.override_rate >= 0.05
    };
    println!("{label} gate: {}", if pass { "PASS" } else { "FAIL" });

    let json = format!(
        "{{\"schema\":\"civvis-q-pairwise-v1\",\"feature_width\":{width},\"replicas\":{replicas},\"keep\":\"{keep}\",\"override_probability\":{threshold:.6},\"weights\":[{}]}}",
        net.weights
            .iter()
            .map(|weight| format!("{weight:.8}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    if let Some(parent) = std::path::Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::File::create(&out)
        .and_then(|mut file| file.write_all(json.as_bytes()))
        .unwrap_or_else(|error| {
            eprintln!("q_pairwise_train: cannot write {out}: {error}");
            std::process::exit(2);
        });
    println!("wrote {out}");
}

#[cfg(test)]
mod tests {
    use super::{evaluate, gated_choice, group_loss, mask, superiority_target, Group, Linear};

    fn group(rows: Vec<Vec<f32>>, returns: Vec<Vec<f32>>) -> Group {
        let means = returns
            .iter()
            .map(|values| values.iter().sum::<f32>() / values.len() as f32)
            .collect();
        Group {
            rows,
            returns,
            means,
            game: 7,
            decision: (8, 1, 9),
        }
    }

    #[test]
    fn jeffreys_target_preserves_doctrine_disagreement() {
        let right = [0.0; 4];
        assert!((superiority_target(&[1.0; 4], &right) - 0.9).abs() < 1e-6);
        assert!((superiority_target(&[1.0, 1.0, 1.0, -1.0], &right) - 0.7).abs() < 1e-6);
        assert!((superiority_target(&[1.0, 1.0, -1.0, -1.0], &right) - 0.5).abs() < 1e-6);
        assert!((superiority_target(&[0.0; 4], &right) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pairwise_gradient_rewards_consistency_and_shrinks_a_split() {
        let consistent = group(vec![vec![1.0], vec![0.0]], vec![vec![1.0; 4], vec![0.0; 4]]);
        let net = Linear::new(1);
        let mut grad = vec![0.0];
        let (_, pairs) = group_loss(&net, &consistent, Some(&mut grad));
        assert_eq!(pairs, 1);
        assert!(grad[0] < 0.0, "descent must raise the consistent winner");

        let split = group(
            vec![vec![1.0], vec![0.0]],
            vec![vec![1.0, 0.0, 1.0, 0.0], vec![0.0, 1.0, 0.0, 1.0]],
        );
        let mut split_grad = vec![0.0];
        group_loss(&net, &split, Some(&mut split_grad));
        assert!(split_grad[0].abs() < 1e-6);
    }

    #[test]
    fn probability_gate_abstains_before_it_overrides() {
        assert_eq!(gated_choice(&[0.0, 0.5], 0.70), 0);
        assert_eq!(gated_choice(&[0.0, 1.0], 0.70), 1);

        let measured = group(vec![vec![0.0], vec![1.0]], vec![vec![0.1; 4], vec![0.2; 4]]);
        let low = evaluate(&Linear { weights: vec![0.5] }, &[measured], 0.70);
        assert_eq!(low[&7].overrides, 0);
    }

    #[test]
    fn destination_mask_uses_the_shared_boundaries() {
        let state = civvis::decision_features::WIDTH;
        let kinds = civvis::action_space::KINDS.len();
        let legacy = civvis::action_space::LEGACY_NUMERIC_WIDTH;
        let width = state + civvis::action_space::FEATURE_WIDTH;
        let destination = state + kinds + legacy;
        let mut row = vec![1.0; width];
        mask(&mut row, "destination");
        assert!(row[..destination].iter().all(|value| *value == 0.0));
        assert!(row[destination..].iter().all(|value| *value == 1.0));
    }

    #[test]
    fn game_macro_lift_does_not_weight_a_long_trajectory_more() {
        let net = Linear { weights: vec![1.0] };
        let mut helped = group(vec![vec![0.0], vec![1.0]], vec![vec![0.1; 4], vec![0.2; 4]]);
        helped.game = 1;
        let mut hurt = group(vec![vec![0.0], vec![1.0]], vec![vec![0.2; 4], vec![0.1; 4]]);
        hurt.game = 2;
        let mut groups = vec![helped];
        groups.extend((0..9).map(|turn| {
            let mut decision = hurt.clone();
            decision.decision.0 += turn;
            decision
        }));
        let games = evaluate(&net, &groups, 0.70);
        let lift = games
            .values()
            .map(|game| game.gated_lift / game.decisions as f32)
            .sum::<f32>()
            / games.len() as f32;
        assert!(lift.abs() < 1e-6, "each independent game must carry half");
    }
}
