//! Calibrate a frozen replica-aware move ranker on independent games.
//!
//! The pairwise destination ranker can order counterfactual moves while its raw
//! logistic margin remains much too compressed for a probability gate. This
//! tool keeps that ranker frozen, fits a monotone two-parameter Platt map on a
//! new Standard corpus, and applies the fixed 0.70 override rule to a disjoint
//! Standard selection corpus. It refuses an optional external evaluation
//! unless the preregistered selection gate passes.
//!
//! ```text
//! q_pairwise_calibrate --model /tmp/q-pairwise-base.json \
//!   --calibration-data /tmp/q-calibration.csv \
//!   --selection-data /tmp/q-selection.csv \
//!   --out /tmp/q-calibrator.json
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};

const EPS: f64 = 1e-9;
const CALIBRATION_SEED: u64 = 948_000;
const CALIBRATION_GAMES: usize = 32;
const SELECTION_SEED: u64 = 948_032;
const SELECTION_GAMES: usize = 32;
const EXTERNAL_SEED: u64 = 947_000;
const EXTERNAL_GAMES: usize = 32;
const REQUIRED_REPLICAS: usize = 4;

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
struct FrozenModel {
    schema: String,
    feature_width: usize,
    replicas: usize,
    keep: String,
    override_probability: f64,
    weights: Vec<f64>,
}

impl FrozenModel {
    fn load(path: &str) -> Result<FrozenModel, String> {
        let source =
            fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
        let model: FrozenModel = serde_json::from_str(&source)
            .map_err(|error| format!("cannot parse {path}: {error}"))?;
        if model.schema != "civvis-q-pairwise-v1" {
            return Err(format!("{path}: unsupported schema {:?}", model.schema));
        }
        if model.keep != "destination"
            || model.replicas != REQUIRED_REPLICAS
            || model.feature_width != model.weights.len()
            || model.weights.iter().any(|weight| !weight.is_finite())
            || (model.override_probability - 0.70).abs() > EPS
        {
            return Err(format!(
                "{path}: expected a finite destination model with {} replicas, width-matched weights, and threshold 0.70",
                REQUIRED_REPLICAS
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

fn mask(row: &mut [f64], keep: &str) {
    let width = row.len();
    let state = width - civvis::action_space::FEATURE_WIDTH;
    let kinds = civvis::action_space::KINDS.len();
    let destination = state + kinds + civvis::action_space::LEGACY_NUMERIC_WIDTH;
    match keep {
        "destination" => {
            for value in row.iter_mut().take(destination) {
                *value = 0.0;
            }
        }
        _ => panic!("unsupported frozen feature block {keep}"),
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
    if replicas != REQUIRED_REPLICAS || group.returns.iter().any(|values| values.len() != replicas)
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
    let raw_width = return_column - 5;
    let expected = civvis::decision_features::WIDTH + civvis::action_space::FEATURE_WIDTH;
    if raw_width != expected {
        return Err(format!(
            "{path}: {raw_width} candidate features do not match current schema {expected}"
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
        let replica_mean = replica_returns.iter().sum::<f64>() / replicas as f64;
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

fn best_sibling(model: &FrozenModel, group: &Group) -> (usize, f64) {
    let expert = model.score(&group.rows[0]);
    let sibling = group.rows.iter().enumerate().skip(1).fold(
        (1, model.score(&group.rows[1])),
        |best, (index, row)| {
            let score = model.score(row);
            if score > best.1 + EPS {
                (index, score)
            } else {
                best
            }
        },
    );
    (sibling.0, sibling.1 - expert)
}

#[derive(Clone, Copy)]
struct Sample {
    game: u64,
    margin: f64,
    target: f64,
}

fn samples(model: &FrozenModel, groups: &[Group]) -> Vec<Sample> {
    groups
        .iter()
        .map(|group| {
            let (sibling, margin) = best_sibling(model, group);
            Sample {
                game: group.game,
                margin,
                target: superiority_target(&group.returns[sibling], &group.returns[0]),
            }
        })
        .collect()
}

fn game_weights(samples: &[Sample]) -> Vec<f64> {
    let mut counts = BTreeMap::<u64, usize>::new();
    for sample in samples {
        *counts.entry(sample.game).or_default() += 1;
    }
    samples
        .iter()
        .map(|sample| 1.0 / counts[&sample.game] as f64)
        .collect()
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Calibrator {
    margin_mean: f64,
    margin_stddev: f64,
    slope: f64,
    intercept: f64,
}

impl Calibrator {
    fn probability(&self, margin: f64) -> f64 {
        let standardized = (margin - self.margin_mean) / self.margin_stddev;
        sigmoid(self.slope * standardized + self.intercept)
    }
}

fn calibration_loss(calibrator: Calibrator, samples: &[Sample], l2: f64) -> f64 {
    let weights = game_weights(samples);
    let games = samples
        .iter()
        .map(|sample| sample.game)
        .collect::<BTreeSet<_>>()
        .len()
        .max(1) as f64;
    let data = samples
        .iter()
        .zip(weights)
        .map(|(sample, weight)| {
            let probability = calibrator.probability(sample.margin).clamp(EPS, 1.0 - EPS);
            -weight
                * (sample.target * probability.ln()
                    + (1.0 - sample.target) * (1.0 - probability).ln())
        })
        .sum::<f64>()
        / games;
    data + 0.5 * l2 * calibrator.slope * calibrator.slope
}

fn fit_calibrator(samples: &[Sample], steps: usize, rate: f64, l2: f64) -> Calibrator {
    let weights = game_weights(samples);
    let games = samples
        .iter()
        .map(|sample| sample.game)
        .collect::<BTreeSet<_>>()
        .len()
        .max(1) as f64;
    let margin_mean = samples
        .iter()
        .zip(&weights)
        .map(|(sample, weight)| sample.margin * weight)
        .sum::<f64>()
        / games;
    let variance = samples
        .iter()
        .zip(&weights)
        .map(|(sample, weight)| weight * (sample.margin - margin_mean).powi(2))
        .sum::<f64>()
        / games;
    let margin_stddev = variance.sqrt().max(1e-6);
    let mut calibrator = Calibrator {
        margin_mean,
        margin_stddev,
        slope: 0.0,
        intercept: 0.0,
    };
    for step in 1..=steps {
        let mut slope_gradient = 0.0;
        let mut intercept_gradient = 0.0;
        for (sample, weight) in samples.iter().zip(&weights) {
            let standardized = (sample.margin - margin_mean) / margin_stddev;
            let residual = calibrator.probability(sample.margin) - sample.target;
            slope_gradient += weight * residual * standardized;
            intercept_gradient += weight * residual;
        }
        slope_gradient = slope_gradient / games + l2 * calibrator.slope;
        intercept_gradient /= games;
        calibrator.slope = (calibrator.slope - rate * slope_gradient).clamp(0.0, 20.0);
        calibrator.intercept -= rate * intercept_gradient;
        if step == 1 || step % 500 == 0 || step == steps {
            println!(
                "calibration step {step:>4}: loss {:.6}, slope {:.4}, intercept {:+.4}",
                calibration_loss(calibrator, samples, l2),
                calibrator.slope,
                calibrator.intercept
            );
        }
    }
    calibrator
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
    calibrated_brier: f64,
    raw_log_loss: f64,
    calibrated_log_loss: f64,
    overrides: usize,
    positive_overrides: usize,
    tied_overrides: usize,
    negative_overrides: usize,
    doctrine_wins: usize,
    doctrine_ties: usize,
    doctrine_losses: usize,
}

fn log_loss(probability: f64, target: f64) -> f64 {
    let probability = probability.clamp(EPS, 1.0 - EPS);
    -target * probability.ln() - (1.0 - target) * (1.0 - probability).ln()
}

fn evaluate(
    model: &FrozenModel,
    calibrator: Calibrator,
    groups: &[Group],
    threshold: f64,
) -> (BTreeMap<u64, GameMetrics>, Vec<f64>) {
    let mut games = BTreeMap::<u64, GameMetrics>::new();
    let mut probabilities = Vec::with_capacity(groups.len());
    for group in groups {
        let scores: Vec<f64> = group.rows.iter().map(|row| model.score(row)).collect();
        let (sibling, margin) = best_sibling(model, group);
        let raw_probability = sigmoid(margin);
        let probability = calibrator.probability(margin);
        probabilities.push(probability);
        let target = superiority_target(&group.returns[sibling], &group.returns[0]);
        let ungated = scores
            .iter()
            .enumerate()
            .skip(1)
            .fold(0, |best, (index, score)| {
                if *score > scores[best] + EPS {
                    index
                } else {
                    best
                }
            });
        let gated = if probability + EPS >= threshold {
            sibling
        } else {
            0
        };
        let best = group
            .means
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let low = group.means.iter().copied().fold(f64::INFINITY, f64::min);
        let chance = group.means.iter().sum::<f64>() / group.means.len() as f64;
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
        metrics.raw_brier += (raw_probability - target).powi(2);
        metrics.calibrated_brier += (probability - target).powi(2);
        metrics.raw_log_loss += log_loss(raw_probability, target);
        metrics.calibrated_log_loss += log_loss(probability, target);
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
    lift: f64,
    lift_se: f64,
    override_rate: f64,
    raw_brier: f64,
    calibrated_brier: f64,
}

fn report(
    model: &FrozenModel,
    calibrator: Calibrator,
    groups: &[Group],
    threshold: f64,
    label: &str,
) -> Report {
    let (games, probabilities) = evaluate(model, calibrator, groups, threshold);
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
    let (raw_brier, raw_brier_se) = mean_se(&metric(&games, |game| game.raw_brier));
    let (calibrated_brier, calibrated_brier_se) =
        mean_se(&metric(&games, |game| game.calibrated_brier));
    let (raw_log_loss, _) = mean_se(&metric(&games, |game| game.raw_log_loss));
    let (calibrated_log_loss, _) = mean_se(&metric(&games, |game| game.calibrated_log_loss));
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
        "  Brier raw {raw_brier:.5} +/- {raw_brier_se:.5}; \
         calibrated {calibrated_brier:.5} +/- {calibrated_brier_se:.5}; \
         log loss {raw_log_loss:.5} -> {calibrated_log_loss:.5}"
    );
    println!(
        "  calibrated P: p50 {:.3}, p90 {:.3}, p99 {:.3}, max {:.3}; \
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
        lift,
        lift_se,
        override_rate,
        raw_brier,
        calibrated_brier,
    }
}

fn selection_pass(report: &Report) -> bool {
    report.calibrated_brier + EPS < report.raw_brier
        && report.lift > 0.0
        && report.override_rate >= 0.05
}

fn external_pass(report: &Report) -> bool {
    report.calibrated_brier + EPS < report.raw_brier
        && report.lift - 1.96 * report.lift_se > 0.0
        && report.override_rate >= 0.05
}

#[derive(Serialize)]
struct Artifact<'a> {
    schema: &'static str,
    base_schema: &'a str,
    base_feature_width: usize,
    base_keep: &'a str,
    base_weights: &'a [f64],
    replicas: usize,
    steps: usize,
    rate: f64,
    l2: f64,
    override_probability: f64,
    calibration: Calibrator,
}

fn write_artifact(
    out: &str,
    model: &FrozenModel,
    calibrator: Calibrator,
    steps: usize,
    rate: f64,
    l2: f64,
    threshold: f64,
) {
    let artifact = Artifact {
        schema: "civvis-q-pairwise-calibration-v1",
        base_schema: &model.schema,
        base_feature_width: model.feature_width,
        base_keep: &model.keep,
        base_weights: &model.weights,
        replicas: model.replicas,
        steps,
        rate,
        l2,
        override_probability: threshold,
        calibration: calibrator,
    };
    if let Some(parent) = std::path::Path::new(out).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let json = serde_json::to_string(&artifact).expect("calibration artifact serializes");
    fs::File::create(out)
        .and_then(|mut file| file.write_all(json.as_bytes()))
        .unwrap_or_else(|error| {
            eprintln!("q_pairwise_calibrate: cannot write {out}: {error}");
            std::process::exit(2);
        });
    println!("wrote {out}");
}

fn load_experiment_data(
    path: &str,
    model: &FrozenModel,
    seed: u64,
    count: usize,
    label: &str,
) -> Loaded {
    let loaded = load_groups(path, &model.keep).unwrap_or_else(|error| {
        eprintln!("q_pairwise_calibrate: {error}");
        std::process::exit(2);
    });
    if loaded.width != model.feature_width || loaded.replicas != model.replicas {
        eprintln!("q_pairwise_calibrate: {label} data/model schemas differ");
        std::process::exit(2);
    }
    validate_games(&loaded.groups, seed, count, label).unwrap_or_else(|error| {
        eprintln!("q_pairwise_calibrate: {error}");
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
    let model_path = text(&args, "--model", "/tmp/q-pairwise-base.json");
    let calibration_path = text(&args, "--calibration-data", "/tmp/q-calibration.csv");
    let selection_path = text(&args, "--selection-data", "/tmp/q-selection.csv");
    let fit_only = args.iter().any(|arg| arg == "--fit-only");
    let external_path = args
        .iter()
        .position(|arg| arg == "--external-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let out = text(&args, "--out", "/tmp/q-calibrator.json");
    let steps = number(&args, "--steps", 4_000);
    let rate = decimal(&args, "--rate", 0.05);
    let l2 = decimal(&args, "--l2", 0.01);
    let threshold = decimal(&args, "--override-probability", 0.70);
    if steps != 4_000
        || (rate - 0.05).abs() > EPS
        || (l2 - 0.01).abs() > EPS
        || (threshold - 0.70).abs() > EPS
    {
        eprintln!(
            "q_pairwise_calibrate: preregistration fixes steps=4000, rate=0.05, l2=0.01, threshold=0.70"
        );
        std::process::exit(2);
    }
    let model = FrozenModel::load(&model_path).unwrap_or_else(|error| {
        eprintln!("q_pairwise_calibrate: {error}");
        std::process::exit(2);
    });
    println!(
        "frozen {model_path}: width {}, {} replicas, keep {}, raw threshold {:.2}",
        model.feature_width, model.replicas, model.keep, model.override_probability
    );
    let calibration = load_experiment_data(
        &calibration_path,
        &model,
        CALIBRATION_SEED,
        CALIBRATION_GAMES,
        "calibration",
    );
    let calibrator = fit_calibrator(&samples(&model, &calibration.groups), steps, rate, l2);
    println!(
        "frozen calibrator: margin mean {:+.6}, stddev {:.6}, slope {:.4}, intercept {:+.4}",
        calibrator.margin_mean, calibrator.margin_stddev, calibrator.slope, calibrator.intercept
    );
    report(
        &model,
        calibrator,
        &calibration.groups,
        threshold,
        "calibration",
    );
    write_artifact(&out, &model, calibrator, steps, rate, l2, threshold);
    if fit_only {
        if external_path.is_some() {
            eprintln!("q_pairwise_calibrate: --fit-only cannot inspect external data");
            std::process::exit(2);
        }
        println!("fit-only: selection data remained unopened");
        return;
    }
    let selection = load_experiment_data(
        &selection_path,
        &model,
        SELECTION_SEED,
        SELECTION_GAMES,
        "selection",
    );
    let selection_report = report(
        &model,
        calibrator,
        &selection.groups,
        threshold,
        "selection",
    );
    let selected = selection_pass(&selection_report);
    println!("selection gate: {}", if selected { "PASS" } else { "FAIL" });

    if let Some(path) = external_path.as_deref() {
        if !selected {
            eprintln!(
                "q_pairwise_calibrate: refusing external data because the Standard selection gate failed"
            );
            std::process::exit(3);
        }
        let external =
            load_experiment_data(path, &model, EXTERNAL_SEED, EXTERNAL_GAMES, "external");
        let external_report = report(&model, calibrator, &external.groups, threshold, "external");
        println!(
            "external gate: {}",
            if external_pass(&external_report) {
                "PASS"
            } else {
                "FAIL"
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fit_calibrator, game_weights, mask, selection_pass, superiority_target, Calibrator, Report,
        Sample,
    };

    #[test]
    fn jeffreys_target_keeps_matched_doctrine_evidence() {
        let expert = [0.0; 4];
        assert!((superiority_target(&[1.0; 4], &expert) - 0.9).abs() < 1e-9);
        assert!((superiority_target(&[1.0, 1.0, 1.0, -1.0], &expert) - 0.7).abs() < 1e-9);
        assert!((superiority_target(&[1.0, 1.0, -1.0, -1.0], &expert) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn calibration_is_monotone_and_expands_a_real_margin_signal() {
        let samples = vec![
            Sample {
                game: 1,
                margin: -0.02,
                target: 0.1,
            },
            Sample {
                game: 2,
                margin: -0.01,
                target: 0.3,
            },
            Sample {
                game: 3,
                margin: 0.01,
                target: 0.7,
            },
            Sample {
                game: 4,
                margin: 0.02,
                target: 0.9,
            },
        ];
        let fitted = fit_calibrator(&samples, 4_000, 0.05, 0.01);
        assert!(fitted.slope > 0.0);
        assert!(fitted.probability(0.02) > 0.70);
        assert!(fitted.probability(-0.02) < 0.30);
    }

    #[test]
    fn game_weights_do_not_let_a_long_game_dominate() {
        let samples = vec![
            Sample {
                game: 1,
                margin: 0.0,
                target: 0.5,
            },
            Sample {
                game: 2,
                margin: 0.0,
                target: 0.5,
            },
            Sample {
                game: 2,
                margin: 0.0,
                target: 0.5,
            },
            Sample {
                game: 2,
                margin: 0.0,
                target: 0.5,
            },
        ];
        let weights = game_weights(&samples);
        assert!((weights[0] - 1.0).abs() < 1e-9);
        assert!((weights[1..].iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn selection_requires_calibration_lift_and_coverage_together() {
        let mut report = Report {
            lift: 0.01,
            lift_se: 0.0,
            override_rate: 0.05,
            raw_brier: 0.10,
            calibrated_brier: 0.09,
        };
        assert!(selection_pass(&report));
        report.override_rate = 0.049;
        assert!(!selection_pass(&report));
        report.override_rate = 0.05;
        report.calibrated_brier = report.raw_brier;
        assert!(!selection_pass(&report));
        report.calibrated_brier = 0.09;
        report.lift = 0.0;
        assert!(!selection_pass(&report));
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
    fn standardized_probability_gate_can_still_abstain() {
        let calibrator = Calibrator {
            margin_mean: 0.0,
            margin_stddev: 0.1,
            slope: 1.0,
            intercept: 0.0,
        };
        assert!(calibrator.probability(0.05) < 0.70);
        assert!(calibrator.probability(0.10) > 0.70);
    }
}
