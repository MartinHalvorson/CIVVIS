//! Learn when a frozen counterfactual move ranker may safely override an expert.
//!
//! Candidate ordering and override reliability are different tasks. This tool
//! keeps the replica-aware destination ranker frozen, names its best sibling,
//! then predicts paired doctrine superiority from absolute state/destination
//! context. Development is game-grouped out-of-fold; blind selection and
//! external files are not opened unless the preceding gate passes.

use civvis::q_override::{
    self, Artifact, GateEvidence, Qualification, ReliabilityModel, DEPLOYMENT_GAMES,
    DEPLOYMENT_SEED, DEVELOPMENT_GAMES, DEVELOPMENT_SEED, OVERRIDE_PROBABILITY,
    RELIABILITY_FEATURES, RELIABILITY_WIDTH, REQUIRED_REPLICAS, SELECTION_GAMES,
    SELECTION_SEED,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};

const EPS: f64 = 1e-9;
const FOLDS: usize = 5;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn decimal(args: &[String], flag: &str, default: f64) -> f64 {
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

#[derive(Deserialize)]
struct FrozenRanker {
    schema: String,
    feature_width: usize,
    replicas: usize,
    keep: String,
    override_probability: f64,
    weights: Vec<f64>,
}

impl FrozenRanker {
    fn load(path: &str) -> Result<FrozenRanker, String> {
        let source =
            fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
        let model: FrozenRanker = serde_json::from_str(&source)
            .map_err(|error| format!("cannot parse {path}: {error}"))?;
        if model.schema != "civvis-q-pairwise-v1"
            || model.keep != "destination"
            || model.replicas != REQUIRED_REPLICAS
            || model.feature_width != model.weights.len()
            || model.weights.iter().any(|weight| !weight.is_finite())
            || (model.override_probability - 0.70).abs() > EPS
        {
            return Err(format!(
                "{path}: expected the finite four-replica destination ranker with threshold 0.70"
            ));
        }
        Ok(model)
    }

    fn score(&self, row: &[f64]) -> f64 {
        self.weights
            .iter()
            .zip(row)
            .map(|(weight, value)| weight * value)
            .sum()
    }
}

#[derive(Clone)]
struct Group {
    rows: Vec<Vec<f64>>,
    returns: Vec<Vec<f64>>,
    means: Vec<f64>,
    game: u64,
    decision: (u32, usize, u32),
}

struct Loaded {
    groups: Vec<Group>,
    width: usize,
    replicas: usize,
    rows: usize,
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
    if group.returns[0].len() != REQUIRED_REPLICAS
        || group
            .returns
            .iter()
            .any(|values| values.len() != REQUIRED_REPLICAS)
    {
        return Err(format!(
            "{path}: game {} decision {:?} must have exactly {REQUIRED_REPLICAS} replicas",
            group.game, group.decision
        ));
    }
    Ok(group)
}

fn parse_value(field: &str, path: &str, line: usize, name: &str) -> Result<f64, String> {
    field
        .parse::<f64>()
        .map_err(|error| format!("{path}:{line}: invalid {name} {field:?}: {error}"))
}

fn load_groups(path: &str) -> Result<Loaded, String> {
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
    let width = return_column - 5;
    let expected = civvis::decision_features::WIDTH + civvis::action_space::FEATURE_WIDTH;
    if width != expected {
        return Err(format!(
            "{path}: {width} candidate features do not match current schema {expected}"
        ));
    }
    let replicas = names.len().saturating_sub(return_column + 1);
    if replicas != REQUIRED_REPLICAS {
        return Err(format!(
            "{path}: expected {REQUIRED_REPLICAS} distinct doctrine replicas, found {replicas}"
        ));
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
        let candidate = fields[5..return_column]
            .iter()
            .enumerate()
            .map(|(index, field)| {
                parse_value(field, path, line_number, &format!("feature {index}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mean = parse_value(fields[return_column], path, line_number, "return")?;
        let returns = fields[return_column + 1..]
            .iter()
            .enumerate()
            .map(|(index, field)| {
                parse_value(field, path, line_number, &format!("replica {index}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if candidate.iter().any(|value| !value.is_finite())
            || !mean.is_finite()
            || returns.iter().any(|value| !value.is_finite())
        {
            return Err(format!("{path}:{line_number}: non-finite value"));
        }
        let replica_mean = returns.iter().sum::<f64>() / returns.len() as f64;
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
                returns: vec![returns],
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
            group.returns.push(returns);
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
        width,
        replicas,
        rows,
    })
}

fn hash(game: u64) -> u64 {
    let mut hash = game.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^ (hash >> 32)
}

fn fold(game: u64) -> usize {
    (hash(game) % FOLDS as u64) as usize
}

fn validate_games(groups: &[Group], seed: u64, count: usize, label: &str) -> Result<(), String> {
    let actual: BTreeSet<u64> = groups.iter().map(|group| group.game).collect();
    let expected: BTreeSet<u64> = (seed..seed + count as u64).collect();
    if actual != expected {
        let missing: Vec<u64> = expected.difference(&actual).copied().collect();
        let extra: Vec<u64> = actual.difference(&expected).copied().collect();
        return Err(format!(
            "{label}: game IDs do not match preregistration; missing {missing:?}, extra {extra:?}"
        ));
    }
    Ok(())
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

fn superiority_target(left: &[f64], right: &[f64]) -> f64 {
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
    (successes + 0.5) / (left.len() as f64 + 1.0)
}

fn best_sibling(ranker: &FrozenRanker, group: &Group) -> (usize, f64) {
    let expert = ranker.score(&group.rows[0]);
    let sibling = group.rows.iter().enumerate().skip(1).fold(
        (1, ranker.score(&group.rows[1])),
        |best, (index, row)| {
            let score = ranker.score(row);
            if score > best.1 + EPS {
                (index, score)
            } else {
                best
            }
        },
    );
    (sibling.0, sibling.1 - expert)
}

#[derive(Clone)]
struct Example {
    game: u64,
    features: Vec<f64>,
    target: f64,
    margin: f64,
    means: Vec<f64>,
    expert_returns: Vec<f64>,
    sibling_returns: Vec<f64>,
    sibling: usize,
}

fn make_example(ranker: &FrozenRanker, group: &Group) -> Example {
    let (sibling, margin) = best_sibling(ranker, group);
    let action = civvis::decision_features::WIDTH;
    let features = q_override::reliability_features(
        &group.rows[0][action..],
        &group.rows[sibling][action..],
        margin,
    )
    .to_vec();
    Example {
        game: group.game,
        features,
        target: superiority_target(&group.returns[sibling], &group.returns[0]),
        margin,
        means: group.means.clone(),
        expert_returns: group.returns[0].clone(),
        sibling_returns: group.returns[sibling].clone(),
        sibling,
    }
}

fn examples(ranker: &FrozenRanker, groups: &[Group]) -> Vec<Example> {
    groups
        .iter()
        .map(|group| make_example(ranker, group))
        .collect()
}

fn game_weights(examples: &[Example]) -> Vec<f64> {
    let mut counts = BTreeMap::<u64, usize>::new();
    for example in examples {
        *counts.entry(example.game).or_default() += 1;
    }
    examples
        .iter()
        .map(|example| 1.0 / counts[&example.game] as f64)
        .collect()
}

fn probability(model: &ReliabilityModel, features: &[f64]) -> f64 {
    let logit = model
        .weights
        .iter()
        .zip(&model.means)
        .zip(&model.stddevs)
        .zip(features)
        .fold(model.intercept, |sum, (((weight, mean), stddev), value)| {
            sum + weight * (value - mean) / stddev
        });
    sigmoid(logit)
}

fn fit(examples: &[Example], steps: usize, rate: f64, l2: f64, quiet: bool) -> ReliabilityModel {
    let sample_weights = game_weights(examples);
    let games = examples
        .iter()
        .map(|example| example.game)
        .collect::<BTreeSet<_>>()
        .len()
        .max(1) as f64;
    let mut means = vec![0.0; RELIABILITY_WIDTH];
    for (example, sample_weight) in examples.iter().zip(&sample_weights) {
        for (mean, value) in means.iter_mut().zip(&example.features) {
            *mean += sample_weight * value / games;
        }
    }
    let mut stddevs = vec![0.0; RELIABILITY_WIDTH];
    for (example, sample_weight) in examples.iter().zip(&sample_weights) {
        for ((variance, value), mean) in stddevs.iter_mut().zip(&example.features).zip(&means) {
            *variance += sample_weight * (value - mean).powi(2) / games;
        }
    }
    for stddev in &mut stddevs {
        *stddev = stddev.sqrt().max(1e-6);
    }
    let constant_probability = examples
        .iter()
        .zip(&sample_weights)
        .map(|(example, weight)| example.target * weight)
        .sum::<f64>()
        / games;
    let mut model = ReliabilityModel {
        means,
        stddevs,
        weights: vec![0.0; RELIABILITY_WIDTH],
        intercept: 0.0,
        constant_probability,
    };
    let mut gradient = vec![0.0; RELIABILITY_WIDTH];
    for step in 1..=steps {
        gradient.fill(0.0);
        let mut intercept_gradient = 0.0;
        let mut loss = 0.0;
        for (example, sample_weight) in examples.iter().zip(&sample_weights) {
            let probability = probability(&model, &example.features);
            let residual = probability - example.target;
            intercept_gradient += sample_weight * residual;
            for (((gradient, mean), stddev), value) in gradient
                .iter_mut()
                .zip(&model.means)
                .zip(&model.stddevs)
                .zip(&example.features)
            {
                *gradient += sample_weight * residual * (value - mean) / stddev;
            }
            loss += sample_weight * log_loss(probability, example.target);
        }
        for (weight, gradient) in model.weights.iter_mut().zip(&gradient) {
            *weight -= rate * (*gradient / games + l2 * *weight);
        }
        model.intercept -= rate * intercept_gradient / games;
        if !quiet && (step == 1 || step % 1_000 == 0 || step == steps) {
            let penalty = 0.5
                * l2
                * model
                    .weights
                    .iter()
                    .map(|weight| weight * weight)
                    .sum::<f64>();
            println!(
                "step {step:>4}: reliability loss {:.6}",
                loss / games + penalty
            );
        }
    }
    model
}

#[derive(Clone, Copy)]
struct Prediction {
    probability: f64,
    constant_probability: f64,
}

fn out_of_fold(examples: &[Example], steps: usize, rate: f64, l2: f64) -> Vec<Prediction> {
    let mut predictions = vec![None; examples.len()];
    for held_fold in 0..FOLDS {
        let train: Vec<Example> = examples
            .iter()
            .filter(|example| fold(example.game) != held_fold)
            .cloned()
            .collect();
        let model = fit(&train, steps, rate, l2, true);
        let train_games = train
            .iter()
            .map(|example| example.game)
            .collect::<BTreeSet<_>>()
            .len();
        let mut held_games = BTreeSet::new();
        let mut held_decisions = 0usize;
        for (index, example) in examples.iter().enumerate() {
            if fold(example.game) == held_fold {
                held_games.insert(example.game);
                held_decisions += 1;
                predictions[index] = Some(Prediction {
                    probability: probability(&model, &example.features),
                    constant_probability: model.constant_probability,
                });
            }
        }
        println!(
            "fold {held_fold}: {train_games} train games -> {} held games / {held_decisions} decisions; base rate {:.3}",
            held_games.len(),
            model.constant_probability
        );
    }
    predictions
        .into_iter()
        .map(|prediction| prediction.expect("every game belongs to one fold"))
        .collect()
}

fn log_loss(probability: f64, target: f64) -> f64 {
    let probability = probability.clamp(EPS, 1.0 - EPS);
    -target * probability.ln() - (1.0 - target) * (1.0 - probability).ln()
}

#[derive(Default)]
struct GameMetrics {
    decisions: usize,
    spread: f64,
    chance_regret: f64,
    expert_regret: f64,
    ungated_regret: f64,
    gated_regret: f64,
    ungated_lift: f64,
    gated_lift: f64,
    raw_brier: f64,
    constant_brier: f64,
    reliability_brier: f64,
    raw_log_loss: f64,
    constant_log_loss: f64,
    reliability_log_loss: f64,
    overrides: usize,
    positive_overrides: usize,
    tied_overrides: usize,
    negative_overrides: usize,
    doctrine_wins: usize,
    doctrine_ties: usize,
    doctrine_losses: usize,
}

fn evaluate(
    examples: &[Example],
    predictions: &[Prediction],
    threshold: f64,
) -> (BTreeMap<u64, GameMetrics>, Vec<f64>) {
    assert_eq!(examples.len(), predictions.len());
    let mut games = BTreeMap::<u64, GameMetrics>::new();
    let mut probabilities = Vec::with_capacity(examples.len());
    for (example, prediction) in examples.iter().zip(predictions) {
        probabilities.push(prediction.probability);
        let raw_probability = sigmoid(example.margin);
        let expert = example.means[0];
        let sibling = example.means[example.sibling];
        let ungated = if example.margin > EPS {
            sibling
        } else {
            expert
        };
        let gated = if prediction.probability + EPS >= threshold {
            sibling
        } else {
            expert
        };
        let best = example
            .means
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let low = example.means.iter().copied().fold(f64::INFINITY, f64::min);
        let chance = example.means.iter().sum::<f64>() / example.means.len() as f64;
        let metrics = games.entry(example.game).or_default();
        metrics.decisions += 1;
        metrics.spread += best - low;
        metrics.chance_regret += best - chance;
        metrics.expert_regret += best - expert;
        metrics.ungated_regret += best - ungated;
        metrics.gated_regret += best - gated;
        metrics.ungated_lift += ungated - expert;
        metrics.gated_lift += gated - expert;
        metrics.raw_brier += (raw_probability - example.target).powi(2);
        metrics.constant_brier += (prediction.constant_probability - example.target).powi(2);
        metrics.reliability_brier += (prediction.probability - example.target).powi(2);
        metrics.raw_log_loss += log_loss(raw_probability, example.target);
        metrics.constant_log_loss += log_loss(prediction.constant_probability, example.target);
        metrics.reliability_log_loss += log_loss(prediction.probability, example.target);
        if prediction.probability + EPS >= threshold {
            metrics.overrides += 1;
            let difference = sibling - expert;
            if difference > EPS {
                metrics.positive_overrides += 1;
            } else if difference < -EPS {
                metrics.negative_overrides += 1;
            } else {
                metrics.tied_overrides += 1;
            }
            for (sibling, expert) in example.sibling_returns.iter().zip(&example.expert_returns) {
                if sibling - expert > EPS {
                    metrics.doctrine_wins += 1;
                } else if expert - sibling > EPS {
                    metrics.doctrine_losses += 1;
                } else {
                    metrics.doctrine_ties += 1;
                }
            }
        }
    }
    probabilities.sort_by(f64::total_cmp);
    (games, probabilities)
}

fn mean_se(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (*value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, (variance / values.len() as f64).sqrt())
}

fn metric(games: &BTreeMap<u64, GameMetrics>, read: impl Fn(&GameMetrics) -> f64) -> Vec<f64> {
    games
        .values()
        .map(|game| read(game) / game.decisions.max(1) as f64)
        .collect()
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * quantile.clamp(0.0, 1.0)).round() as usize;
    sorted[index]
}

struct Report {
    games: usize,
    decisions: usize,
    lift: f64,
    lift_se: f64,
    override_rate: f64,
    raw_brier: f64,
    constant_brier: f64,
    reliability_brier: f64,
}

fn report(examples: &[Example], predictions: &[Prediction], threshold: f64, label: &str) -> Report {
    let (games, probabilities) = evaluate(examples, predictions, threshold);
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
    let (override_rate, override_rate_se) = mean_se(&metric(&games, |game| game.overrides as f64));
    let (raw_brier, _) = mean_se(&metric(&games, |game| game.raw_brier));
    let (constant_brier, _) = mean_se(&metric(&games, |game| game.constant_brier));
    let (reliability_brier, reliability_brier_se) =
        mean_se(&metric(&games, |game| game.reliability_brier));
    let (raw_log_loss, _) = mean_se(&metric(&games, |game| game.raw_log_loss));
    let (constant_log_loss, _) = mean_se(&metric(&games, |game| game.constant_log_loss));
    let (reliability_log_loss, _) = mean_se(&metric(&games, |game| game.reliability_log_loss));
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
        "  Brier raw/constant/reliability {raw_brier:.5}/{constant_brier:.5}/\
         {reliability_brier:.5} +/- {reliability_brier_se:.5}; \
         log loss {raw_log_loss:.5}/{constant_log_loss:.5}/{reliability_log_loss:.5}"
    );
    println!(
        "  reliability P: p50 {:.3}, p90 {:.3}, p99 {:.3}, max {:.3}; \
         {overrides}/{decisions} overrides ({:.1}% game-macro +/- {:.1}%)",
        percentile(&probabilities, 0.50),
        percentile(&probabilities, 0.90),
        percentile(&probabilities, 0.99),
        percentile(&probabilities, 1.00),
        100.0 * override_rate,
        100.0 * override_rate_se
    );
    println!(
        "  override mean outcomes +/=/− {positive}/{tied}/{negative}; \
         doctrine outcomes +/=/− {doctrine_wins}/{doctrine_ties}/{doctrine_losses}"
    );
    Report {
        games: games.len(),
        decisions,
        lift,
        lift_se,
        override_rate,
        raw_brier,
        constant_brier,
        reliability_brier,
    }
}

fn gate_evidence(
    report: &Report,
    profile: &str,
    seed: u64,
    games: usize,
    passed: bool,
) -> GateEvidence {
    assert_eq!(report.games, games);
    GateEvidence {
        profile: profile.to_string(),
        seed,
        games,
        decisions: report.decisions,
        passed,
        raw_brier: report.raw_brier,
        constant_brier: report.constant_brier,
        reliability_brier: report.reliability_brier,
        lift: report.lift,
        lift_se: report.lift_se,
        override_rate: report.override_rate,
    }
}

fn standard_pass(report: &Report) -> bool {
    report.reliability_brier + EPS < report.raw_brier
        && report.reliability_brier + EPS < report.constant_brier
        && report.lift > 0.0
        && report.override_rate >= 0.05
}

fn external_pass(report: &Report) -> bool {
    report.reliability_brier + EPS < report.raw_brier
        && report.reliability_brier + EPS < report.constant_brier
        && report.lift - 1.96 * report.lift_se > 0.0
        && report.override_rate >= 0.05
}

fn write_artifact(
    path: &str,
    ranker: &FrozenRanker,
    model: &ReliabilityModel,
    steps: usize,
    rate: f64,
    l2: f64,
    threshold: f64,
    development: GateEvidence,
    selection: GateEvidence,
    deployment: GateEvidence,
) {
    let artifact = Artifact {
        schema: q_override::SCHEMA.to_string(),
        ranker_schema: ranker.schema.clone(),
        ranker_fingerprint: q_override::ranker_fingerprint(
            &ranker.weights,
            ranker.feature_width,
            ranker.replicas,
        ),
        ranker_weights: ranker.weights.clone(),
        ranker_feature_width: ranker.feature_width,
        reliability_features: RELIABILITY_FEATURES.map(str::to_string).to_vec(),
        reliability_feature_width: RELIABILITY_WIDTH,
        replicas: ranker.replicas,
        folds: FOLDS,
        steps,
        rate,
        l2,
        override_probability: threshold,
        reliability: model.clone(),
        qualification: Qualification {
            status: "qualified".to_string(),
            development,
            selection,
            deployment,
        },
    };
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let json = serde_json::to_vec(&artifact).expect("override artifact serializes");
    let destination = std::path::Path::new(path);
    let temporary = destination.with_extension("qualified.tmp");
    fs::File::create(&temporary)
        .and_then(|mut file| {
            file.write_all(&json)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, destination))
        .unwrap_or_else(|error| {
            eprintln!("q_override_train: cannot write {path}: {error}");
            std::process::exit(2);
        });
    q_override::QualifiedQOverride::load(path).unwrap_or_else(|error| {
        eprintln!("q_override_train: wrote an unloadable artifact: {error}");
        std::process::exit(2);
    });
    println!("wrote {path}");
}

fn load_experiment_data(
    path: &str,
    ranker: &FrozenRanker,
    seed: u64,
    count: usize,
    label: &str,
) -> Loaded {
    let loaded = load_groups(path).unwrap_or_else(|error| {
        eprintln!("q_override_train: {error}");
        std::process::exit(2);
    });
    if loaded.width != ranker.feature_width || loaded.replicas != ranker.replicas {
        eprintln!("q_override_train: {label} data/ranker schemas differ");
        std::process::exit(2);
    }
    validate_games(&loaded.groups, seed, count, label).unwrap_or_else(|error| {
        eprintln!("q_override_train: {error}");
        std::process::exit(2);
    });
    println!(
        "{label} {path}: {} rows -> {} decisions in {count} exact games",
        loaded.rows,
        loaded.groups.len()
    );
    loaded
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ranker_path = text(&args, "--ranker", "/tmp/q-pairwise-base.json");
    let development_path = text(
        &args,
        "--development-data",
        "/tmp/q-override-development.csv",
    );
    let selection_path = args
        .iter()
        .position(|arg| arg == "--selection-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let deployment_path = args
        .iter()
        .position(|arg| arg == "--deployment-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let out = text(&args, "--out", "/tmp/q-override-qualified.json");
    let steps = number(&args, "--steps", 6_000);
    let rate = decimal(&args, "--rate", 0.05);
    let l2 = decimal(&args, "--l2", 0.02);
    let threshold = decimal(&args, "--override-probability", 0.70);
    if steps != 6_000
        || (rate - 0.05).abs() > EPS
        || (l2 - 0.02).abs() > EPS
        || (threshold - OVERRIDE_PROBABILITY).abs() > EPS
    {
        eprintln!(
            "q_override_train: preregistration fixes steps=6000, rate=0.05, l2=0.02, threshold=0.70"
        );
        std::process::exit(2);
    }
    let ranker = FrozenRanker::load(&ranker_path).unwrap_or_else(|error| {
        eprintln!("q_override_train: {error}");
        std::process::exit(2);
    });
    println!(
        "frozen {ranker_path}: width {}, {} replicas, keep {}, raw threshold {:.2}",
        ranker.feature_width, ranker.replicas, ranker.keep, ranker.override_probability
    );
    let development_data = load_experiment_data(
        &development_path,
        &ranker,
        DEVELOPMENT_SEED,
        DEVELOPMENT_GAMES,
        "development",
    );
    let development = examples(&ranker, &development_data.groups);
    println!(
        "development: {} decisions in {} games, fixed reliability width {RELIABILITY_WIDTH}",
        development.len(),
        development
            .iter()
            .map(|example| example.game)
            .collect::<BTreeSet<_>>()
            .len()
    );
    let oof_predictions = out_of_fold(&development, steps, rate, l2);
    let oof_report = report(&development, &oof_predictions, threshold, "out-of-fold");
    let oof_pass = standard_pass(&oof_report);
    println!(
        "out-of-fold gate: {}",
        if oof_pass { "PASS" } else { "FAIL" }
    );

    let Some(selection_path) = selection_path.as_deref() else {
        if deployment_path.is_some() {
            eprintln!("q_override_train: deployment data requires a passing selection corpus");
            std::process::exit(2);
        }
        println!("selection and deployment data remained unopened; no artifact written");
        return;
    };
    if !oof_pass {
        eprintln!("q_override_train: refusing selection data because the out-of-fold gate failed");
        std::process::exit(3);
    }
    println!("fitting frozen all-development reliability head");
    let model = fit(&development, steps, rate, l2, false);
    println!(
        "full model: constant {:.3}, weight L2 {:.4}, largest |weight| {:.4}",
        model.constant_probability,
        model
            .weights
            .iter()
            .map(|weight| weight * weight)
            .sum::<f64>()
            .sqrt(),
        model
            .weights
            .iter()
            .map(|weight| weight.abs())
            .fold(0.0, f64::max)
    );
    let selection = load_experiment_data(
        selection_path,
        &ranker,
        SELECTION_SEED,
        SELECTION_GAMES,
        "selection",
    );
    let selection_examples = examples(&ranker, &selection.groups);
    let selection_predictions: Vec<Prediction> = selection_examples
        .iter()
        .map(|example| Prediction {
            probability: probability(&model, &example.features),
            constant_probability: model.constant_probability,
        })
        .collect();
    let selection_report = report(
        &selection_examples,
        &selection_predictions,
        threshold,
        "selection",
    );
    let selection_pass = standard_pass(&selection_report);
    println!(
        "selection gate: {}",
        if selection_pass { "PASS" } else { "FAIL" }
    );

    if !selection_pass {
        eprintln!(
            "q_override_train: refusing deployment data because the Standard selection gate failed"
        );
        std::process::exit(3);
    }
    let Some(deployment_path) = deployment_path.as_deref() else {
        println!("deployment data remained unopened; no artifact written");
        return;
    };
    let deployment = load_experiment_data(
        deployment_path,
        &ranker,
        DEPLOYMENT_SEED,
        DEPLOYMENT_GAMES,
        "deployment",
    );
    let deployment_examples = examples(&ranker, &deployment.groups);
    let deployment_predictions: Vec<Prediction> = deployment_examples
        .iter()
        .map(|example| Prediction {
            probability: probability(&model, &example.features),
            constant_probability: model.constant_probability,
        })
        .collect();
    let deployment_report = report(
        &deployment_examples,
        &deployment_predictions,
        threshold,
        "deployment",
    );
    let deployment_pass = external_pass(&deployment_report);
    println!(
        "deployment gate: {}",
        if deployment_pass { "PASS" } else { "FAIL" }
    );
    if !deployment_pass {
        eprintln!("q_override_train: deployment gate failed; no artifact written");
        std::process::exit(3);
    }

    write_artifact(
        &out,
        &ranker,
        &model,
        steps,
        rate,
        l2,
        threshold,
        gate_evidence(
            &oof_report,
            "standard_development_oof",
            DEVELOPMENT_SEED,
            DEVELOPMENT_GAMES,
            oof_pass,
        ),
        gate_evidence(
            &selection_report,
            "standard_selection",
            SELECTION_SEED,
            SELECTION_GAMES,
            selection_pass,
        ),
        gate_evidence(
            &deployment_report,
            "online_deployment",
            DEPLOYMENT_SEED,
            DEPLOYMENT_GAMES,
            deployment_pass,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        fit, fold, game_weights, probability, standard_pass, superiority_target, Example, Report,
        RELIABILITY_WIDTH,
    };

    fn example(game: u64, value: f64, target: f64) -> Example {
        Example {
            game,
            features: vec![value; RELIABILITY_WIDTH],
            target,
            margin: value,
            means: vec![0.0, target],
            expert_returns: vec![0.0; 4],
            sibling_returns: vec![target; 4],
            sibling: 1,
        }
    }

    #[test]
    fn every_game_belongs_to_exactly_one_fold() {
        for game in 946_000..948_064 {
            assert!(fold(game) < 5);
            assert_eq!(fold(game), fold(game));
        }
    }

    #[test]
    fn jeffreys_target_keeps_doctrine_disagreement() {
        let expert = [0.0; 4];
        assert!((superiority_target(&[1.0; 4], &expert) - 0.9).abs() < 1e-9);
        assert!((superiority_target(&[1.0, 1.0, 1.0, -1.0], &expert) - 0.7).abs() < 1e-9);
        assert!((superiority_target(&[1.0, 1.0, -1.0, -1.0], &expert) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn reliability_head_learns_a_context_signal() {
        let training = vec![
            example(1, -1.0, 0.1),
            example(2, -0.5, 0.3),
            example(3, 0.5, 0.7),
            example(4, 1.0, 0.9),
        ];
        let model = fit(&training, 6_000, 0.05, 0.02, true);
        assert!(probability(&model, &training[3].features) > 0.70);
        assert!(probability(&model, &training[0].features) < 0.30);
    }

    #[test]
    fn game_weights_keep_long_trajectories_from_dominating() {
        let training = vec![
            example(1, 0.0, 0.5),
            example(2, 0.0, 0.5),
            example(2, 0.0, 0.5),
            example(2, 0.0, 0.5),
        ];
        let weights = game_weights(&training);
        assert!((weights[0] - 1.0).abs() < 1e-9);
        assert!((weights[1..].iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn development_gate_requires_calibration_lift_and_coverage() {
        let mut report = Report {
            games: 10,
            decisions: 20,
            lift: 0.01,
            lift_se: 0.0,
            override_rate: 0.05,
            raw_brier: 0.10,
            constant_brier: 0.11,
            reliability_brier: 0.09,
        };
        assert!(standard_pass(&report));
        report.override_rate = 0.049;
        assert!(!standard_pass(&report));
        report.override_rate = 0.05;
        report.reliability_brier = report.raw_brier;
        assert!(!standard_pass(&report));
        report.reliability_brier = 0.09;
        report.lift = 0.0;
        assert!(!standard_pass(&report));
    }
}
