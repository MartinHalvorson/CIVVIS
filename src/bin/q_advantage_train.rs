//! Fit a low-capacity move ranker to counterfactual continuation returns.
//!
//! Every group from `q_counterfactual` is one state and one unit, so state,
//! actor, and action kind are controlled. The target is a softmax over measured
//! candidate returns rather than the expert's action. A linear head is
//! intentional: the first causal dataset is small, while the 35 destination
//! features already express progress, force balance, terrain, and threat. A
//! model that needs thousands of nonlinear parameters to fit a few dozen maps
//! has not shown transferable action credit.
//!
//! Evaluation is grouped by unseen game. It reports regret to the measured
//! oracle and improvement over both random choice and the expert's recorded
//! choice. An external file is never used for model selection; epoch count and
//! target temperature are fixed inputs.
//!
//! ```text
//! q_advantage_train --data /tmp/q-standard.csv \
//!   --eval-data /tmp/q-online.csv --keep destination \
//!   --out /tmp/q-advantage.json
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

struct Group {
    rows: Vec<Vec<f32>>,
    returns: Vec<f32>,
    game: u64,
    decision: (u32, usize, u32), // turn, seat, unit
}

struct Loaded {
    groups: Vec<Group>,
    width: usize,
    rows: usize,
    dropped: usize,
}

/// Match `q_train`'s feature-block ablations so the representation result and
/// causal-return result can be compared without redefining a block.
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
        _ => {}
    }
}

/// Append only the interaction the representation's contract calls for: the
/// eight actor-role flags times the remaining 27 destination quantities. The
/// base terms stay present. This is much smaller and more interpretable than a
/// hidden network, but lets threat, cohesion, and objective progress mean
/// different things for a civilian, scout, ranged unit, or siege train.
fn expand(row: &mut Vec<f32>, interactions: &str) {
    if interactions != "role" {
        return;
    }
    let state = civvis::decision_features::WIDTH;
    let destination =
        state + civvis::action_space::KINDS.len() + civvis::action_space::LEGACY_NUMERIC_WIDTH;
    let roles =
        row[destination..destination + civvis::action_space::DESTINATION_ROLE_WIDTH].to_vec();
    let quantities = row[destination + civvis::action_space::DESTINATION_ROLE_WIDTH
        ..destination + civvis::action_space::DESTINATION_WIDTH]
        .to_vec();
    row.reserve(roles.len() * quantities.len());
    for role in roles {
        row.extend(quantities.iter().map(|quantity| role * quantity));
    }
}

fn load_groups(
    path: &str,
    keep: &str,
    interactions: &str,
    min_spread: f32,
) -> Result<Loaded, String> {
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
    let interaction_width = if interactions == "role" {
        civvis::action_space::DESTINATION_ROLE_WIDTH
            * (civvis::action_space::DESTINATION_WIDTH
                - civvis::action_space::DESTINATION_ROLE_WIDTH)
    } else {
        0
    };
    let width = raw_width + interaction_width;

    let mut groups = Vec::new();
    let mut current: Option<Group> = None;
    let mut rows = 0usize;
    let mut dropped = 0usize;
    let finish = |group: Group, groups: &mut Vec<Group>, dropped: &mut usize| {
        if group.rows.len() < 2 || group.rows.len() != group.returns.len() {
            *dropped += 1;
            return;
        }
        let low = group.returns.iter().copied().fold(f32::INFINITY, f32::min);
        let high = group
            .returns
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        if high - low + EPS < min_spread {
            *dropped += 1;
        } else {
            groups.push(group);
        }
    };

    for line in reader.lines().map_while(Result::ok) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != names.len() {
            dropped += 1;
            continue;
        }
        rows += 1;
        let game = fields[0].parse::<u64>().unwrap_or(0);
        let decision = (
            fields[1].parse::<u32>().unwrap_or(0),
            fields[2].parse::<usize>().unwrap_or(usize::MAX),
            fields[3].parse::<u32>().unwrap_or(0),
        );
        let chosen = fields[4] == "1";
        let value = fields[return_column].parse::<f32>().unwrap_or(0.0);
        let mut candidate: Vec<f32> = fields[5..return_column]
            .iter()
            .map(|field| field.parse().unwrap_or(0.0))
            .collect();
        mask(&mut candidate, keep);
        expand(&mut candidate, interactions);
        if chosen {
            if let Some(group) = current.take() {
                finish(group, &mut groups, &mut dropped);
            }
            current = Some(Group {
                rows: vec![candidate],
                returns: vec![value],
                game,
                decision,
            });
        } else if let Some(group) = current.as_mut() {
            if group.game != game || group.decision != decision {
                dropped += 1;
                continue;
            }
            group.rows.push(candidate);
            group.returns.push(value);
        } else {
            dropped += 1;
        }
    }
    if let Some(group) = current.take() {
        finish(group, &mut groups, &mut dropped);
    }
    Ok(Loaded {
        groups,
        width,
        rows,
        dropped,
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

fn softmax(values: &[f32]) -> Vec<f32> {
    let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut probabilities: Vec<f32> = values.iter().map(|value| (*value - high).exp()).collect();
    let total = probabilities.iter().sum::<f32>().max(1e-30);
    probabilities.iter_mut().for_each(|value| *value /= total);
    probabilities
}

/// Listwise cross-entropy against a temperature-scaled distribution of actual
/// returns. Equal-return candidates get equal credit; larger causal gaps exert
/// more pressure without treating a 0.5001/0.5000 pair as certain truth.
fn group_loss(net: &Linear, group: &Group, temperature: f32, grad: Option<&mut [f32]>) -> f32 {
    let scores: Vec<f32> = group.rows.iter().map(|row| net.score(row)).collect();
    let predicted = softmax(&scores);
    let scaled: Vec<f32> = group
        .returns
        .iter()
        .map(|value| value / temperature.max(1e-5))
        .collect();
    let target = softmax(&scaled);
    let loss = target
        .iter()
        .zip(&predicted)
        .map(|(target, predicted)| -target * predicted.max(1e-30).ln())
        .sum();
    if let Some(grad) = grad {
        for ((row, predicted), target) in group.rows.iter().zip(&predicted).zip(&target) {
            let seed = predicted - target;
            for (gradient, value) in grad.iter_mut().zip(row) {
                *gradient += seed * value;
            }
        }
    }
    loss
}

#[derive(Default)]
struct GameMetrics {
    decisions: usize,
    model_top: f32,
    expert_top: f32,
    chance_top: f32,
    model_regret: f32,
    expert_regret: f32,
    chance_regret: f32,
    spread: f32,
}

fn evaluate(net: &Linear, groups: &[Group]) -> BTreeMap<u64, GameMetrics> {
    let mut games = BTreeMap::<u64, GameMetrics>::new();
    for group in groups {
        let scores: Vec<f32> = group.rows.iter().map(|row| net.score(row)).collect();
        let best = group
            .returns
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let low = group.returns.iter().copied().fold(f32::INFINITY, f32::min);
        let top_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let predicted: Vec<usize> = scores
            .iter()
            .enumerate()
            .filter_map(|(index, score)| ((*score - top_score).abs() <= EPS).then_some(index))
            .collect();
        let actual_best = |index: usize| (group.returns[index] - best).abs() <= EPS;
        let model_return = predicted
            .iter()
            .map(|index| group.returns[*index])
            .sum::<f32>()
            / predicted.len().max(1) as f32;
        let chance_return = group.returns.iter().sum::<f32>() / group.returns.len() as f32;
        let metrics = games.entry(group.game).or_default();
        metrics.decisions += 1;
        metrics.model_top += predicted
            .iter()
            .filter(|index| actual_best(**index))
            .count() as f32
            / predicted.len().max(1) as f32;
        metrics.expert_top += actual_best(0) as u8 as f32;
        metrics.chance_top += group
            .returns
            .iter()
            .enumerate()
            .filter(|(index, _)| actual_best(*index))
            .count() as f32
            / group.returns.len() as f32;
        metrics.model_regret += best - model_return;
        metrics.expert_regret += best - group.returns[0];
        metrics.chance_regret += best - chance_return;
        metrics.spread += best - low;
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

fn report(net: &Linear, groups: &[Group], label: &str) {
    let games = evaluate(net, groups);
    let decisions: usize = games.values().map(|game| game.decisions).sum();
    let (model_top, model_top_se) = mean_se(&metric(&games, |game| game.model_top));
    let (expert_top, _) = mean_se(&metric(&games, |game| game.expert_top));
    let (chance_top, _) = mean_se(&metric(&games, |game| game.chance_top));
    let (model_regret, _) = mean_se(&metric(&games, |game| game.model_regret));
    let (expert_regret, _) = mean_se(&metric(&games, |game| game.expert_regret));
    let (chance_regret, _) = mean_se(&metric(&games, |game| game.chance_regret));
    let (spread, _) = mean_se(&metric(&games, |game| game.spread));
    let (vs_chance, vs_chance_se) = mean_se(&metric(&games, |game| {
        game.chance_regret - game.model_regret
    }));
    let (vs_expert, vs_expert_se) = mean_se(&metric(&games, |game| {
        game.expert_regret - game.model_regret
    }));
    println!(
        "{label}: {decisions} decisions in {} games, mean return spread {spread:.4}",
        games.len()
    );
    println!(
        "  top-return choice: chance {:>5.1}%  expert {:>5.1}%  model {:>5.1} +/- {:>4.1}%",
        100.0 * chance_top,
        100.0 * expert_top,
        100.0 * model_top,
        100.0 * model_top_se
    );
    println!(
        "  oracle regret:     chance {chance_regret:.4}  expert {expert_regret:.4}  model {model_regret:.4}"
    );
    println!(
        "  model return lift: vs chance {vs_chance:+.4} +/- {vs_chance_se:.4}; \
         vs expert {vs_expert:+.4} +/- {vs_expert_se:.4}"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = text(&args, "--data", "/tmp/q-counterfactual.csv");
    let eval_data = args
        .iter()
        .position(|arg| arg == "--eval-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let out = text(&args, "--out", "/tmp/q-advantage.json");
    let keep = text(&args, "--keep", "destination");
    let interactions = text(&args, "--interactions", "none");
    let epochs = number(&args, "--epochs", 40);
    let batch = number(&args, "--batch", 32).max(1);
    let rate = decimal(&args, "--rate", 0.05);
    let temperature = decimal(&args, "--temperature", 0.01).max(1e-5);
    let l2 = decimal(&args, "--l2", 0.0001).max(0.0);
    let holdout_share = decimal(&args, "--holdout", 0.25).clamp(0.0, 1.0);
    let min_spread = decimal(&args, "--min-spread", 0.0).max(0.0);

    if !matches!(interactions.as_str(), "none" | "role") {
        eprintln!("q_advantage_train: --interactions must be none or role");
        std::process::exit(2);
    }
    let loaded = load_groups(&data, &keep, &interactions, min_spread).unwrap_or_else(|error| {
        eprintln!("q_advantage_train: {error}");
        std::process::exit(2);
    });
    println!(
        "{data}: {} rows -> {} decisions, width {}, keep {keep}, interactions {interactions}, dropped {}",
        loaded.rows,
        loaded.groups.len(),
        loaded.width,
        loaded.dropped
    );
    let width = loaded.width;
    let (train, valid, label) = if let Some(path) = eval_data.as_deref() {
        let external =
            load_groups(path, &keep, &interactions, min_spread).unwrap_or_else(|error| {
                eprintln!("q_advantage_train: {error}");
                std::process::exit(2);
            });
        if external.width != width {
            eprintln!("q_advantage_train: train/evaluation widths differ");
            std::process::exit(2);
        }
        println!(
            "external {path}: {} rows -> {} decisions, dropped {}",
            external.rows,
            external.groups.len(),
            external.dropped
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
    if train.is_empty() || valid.is_empty() {
        eprintln!("q_advantage_train: need both training and evaluation games");
        std::process::exit(2);
    }
    println!(
        "{} train decisions, {} {label} decisions; temperature {temperature:.4}, fixed {epochs} epochs",
        train.len(),
        valid.len()
    );

    let mut net = Linear::new(width);
    let mut grad = vec![0.0f32; width];
    let mut order: Vec<usize> = (0..train.len()).collect();
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
        let mut pending = 0usize;
        for (step, index) in order.iter().enumerate() {
            loss += group_loss(&net, &train[*index], temperature, Some(&mut grad));
            pending += 1;
            if pending == batch || step + 1 == order.len() {
                let scale = rate / pending as f32;
                for (weight, gradient) in net.weights.iter_mut().zip(&mut grad) {
                    *weight -= scale * (*gradient + l2 * *weight * pending as f32);
                    *gradient = 0.0;
                }
                pending = 0;
            }
        }
        if epoch == 1 || epoch % 10 == 0 || epoch == epochs {
            println!(
                "epoch {epoch:>3}: train listwise loss {:.4}",
                loss / train.len() as f32
            );
        }
    }
    report(&net, &train, "train");
    report(&net, &valid, label);

    let json = format!(
        "{{\"schema\":\"civvis-q-advantage-v1\",\"feature_width\":{width},\"keep\":\"{keep}\",\"interactions\":\"{interactions}\",\"weights\":[{}]}}",
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
            eprintln!("q_advantage_train: cannot write {out}: {error}");
            std::process::exit(2);
        });
    println!("wrote {out}");
}

#[cfg(test)]
mod tests {
    use super::{evaluate, expand, group_loss, mask, Group, Linear};

    #[test]
    fn return_softmax_pushes_the_better_candidate_up() {
        let group = Group {
            rows: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            returns: vec![0.20, 0.25],
            game: 1,
            decision: (3, 0, 4),
        };
        let net = Linear::new(2);
        let mut grad = vec![0.0; 2];
        let loss = group_loss(&net, &group, 0.01, Some(&mut grad));
        assert!(loss.is_finite());
        assert!(grad[0] > 0.0, "descent must lower the worse score");
        assert!(grad[1] < 0.0, "descent must raise the better score");
    }

    #[test]
    fn evaluation_measures_return_not_expert_imitation() {
        let group = Group {
            rows: vec![vec![0.0], vec![1.0], vec![2.0]],
            returns: vec![0.20, 0.30, 0.25],
            game: 7,
            decision: (8, 1, 9),
        };
        let net = Linear { weights: vec![1.0] };
        let games = evaluate(&net, &[group]);
        let measured = &games[&7];
        assert!((measured.expert_regret - 0.10).abs() < 1e-6);
        assert!((measured.model_regret - 0.05).abs() < 1e-6);
        assert_eq!(measured.expert_top, 0.0);
        assert_eq!(measured.model_top, 0.0);
    }

    #[test]
    fn destination_ablation_uses_the_shared_block_boundaries() {
        let state = civvis::decision_features::WIDTH;
        let kinds = civvis::action_space::KINDS.len();
        let legacy = civvis::action_space::LEGACY_NUMERIC_WIDTH;
        let width = state + civvis::action_space::FEATURE_WIDTH;
        let destination = state + kinds + legacy;
        let plan = destination + civvis::action_space::PLAN_OFFSET;
        let plan_end = plan + civvis::action_space::PLAN_WIDTH;

        let mut only_destination = vec![1.0; width];
        mask(&mut only_destination, "destination");
        assert!(only_destination[..destination]
            .iter()
            .all(|value| *value == 0.0));
        assert!(only_destination[destination..]
            .iter()
            .all(|value| *value == 1.0));

        let mut no_plan = vec![1.0; width];
        mask(&mut no_plan, "destination-no-plan");
        assert!(no_plan[plan..plan_end].iter().all(|value| *value == 0.0));
        assert!(no_plan[destination..plan].iter().all(|value| *value == 1.0));
        assert!(no_plan[plan_end..].iter().all(|value| *value == 1.0));
    }

    #[test]
    fn role_interactions_are_only_role_times_destination_quantity() {
        let state = civvis::decision_features::WIDTH;
        let destination =
            state + civvis::action_space::KINDS.len() + civvis::action_space::LEGACY_NUMERIC_WIDTH;
        let mut row = vec![0.0; state + civvis::action_space::FEATURE_WIDTH];
        row[destination] = 1.0;
        row[destination + civvis::action_space::DESTINATION_ROLE_WIDTH] = 0.25;
        let original = row.len();
        expand(&mut row, "role");
        let quantities =
            civvis::action_space::DESTINATION_WIDTH - civvis::action_space::DESTINATION_ROLE_WIDTH;
        assert_eq!(
            row.len(),
            original + civvis::action_space::DESTINATION_ROLE_WIDTH * quantities
        );
        assert_eq!(row[original], 0.25);
        assert!(row[original + quantities..]
            .iter()
            .all(|value| *value == 0.0));
    }
}
