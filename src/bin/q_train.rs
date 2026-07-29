//! Can this encoding represent the expert's policy at all?
//!
//! Before anything is wired into an agent, one cheap question gates the whole
//! action-conditioned programme: given a state and the set of actions that were
//! legal in it, can a head trained on `q_dataset` rows pick the action
//! `AdvancedAi` actually took? With `--negatives 4` each decision is one chosen
//! action against four that were not, so **20% is chance**. A head that cannot
//! beat chance on held-out games has shown that the encoding does not carry the
//! expert's decision, and no downstream use of it — prior, Q,
//! advantage — can work. That costs one training run to find out instead of a
//! 120-map tournament, which is why it is the first thing run.
//!
//! This deliberately trains a **ranker**, not a greedy value head. The failure
//! it exists to avoid is documented in `src/policy.rs`: a net fit to outcomes
//! encodes correlation, and an argmax over siblings optimises whichever
//! correlate is cheapest to move — which cost 313 Elo. A ranker fit to *which
//! action was taken* has no such freedom. It is asked one question and scored on
//! that question, and its honest ceiling is the expert it imitates. That is the
//! right shape for a search prior, which is where the returns in this codebase
//! have actually been.
//!
//! ```text
//! q_train --data evolved/q.csv --out evolved/qnet.json --epochs 6
//! q_train --data evolved/q-standard.csv \
//!   --eval-data evolved/q-deployment.csv --same-kind --keep geometry
//! q_train --data evolved/q-standard.csv \
//!   --eval-data evolved/q-deployment.csv --same-kind --keep destination
//! ```
//!
//! **Splitting is by game and nothing else.** A per-sample split of the previous
//! value-net data read 98.8% where a per-game split read 75.0%. Rows inside one
//! game share an outcome, a map and an opponent set; any split that mixes them
//! measures memorisation. Games are assigned by hashing the game id, so the same
//! game lands on the same side every run. `--eval-data` is the stronger
//! profile-transfer test: every decision in the primary file trains and every
//! decision in the second file validates, so no deployment map leaks into fit.
//!
//! **Groups are runs, by the emitter's contract.** `q_dataset` writes a chosen
//! row and then its negatives, so a group begins at each `chosen=1`. That is
//! pinned by a test over there rather than assumed here.
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};

const HIDDEN: [usize; 2] = [64, 32];

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

/// One decision: the candidate rows, with the chosen one first.
struct Group {
    rows: Vec<Vec<f32>>,
    game: u64,
    won: f32,
    /// Kind of the chosen action. With mixed-kind negatives this is still the
    /// decision being imitated; with `--same-kind` it names every candidate.
    kind: usize,
}

struct Loaded {
    groups: Vec<Group>,
    columns: usize,
    width: usize,
    lines: usize,
    dropped: usize,
}

/// Blank the blocks this run is not allowed to see. `state` is the leading 34,
/// then action kind, the legacy thirteen scalars, and the appended destination
/// block. `legacy-geometry` and `destination` are the pre-registered ablation:
/// both read the same rows, labels, candidates, and split.
fn mask(row: &mut [f32], keep: &str, width: usize) {
    let state = width - civvis::action_space::FEATURE_WIDTH;
    let kinds = civvis::action_space::KINDS.len();
    let legacy = state + kinds;
    let destination = legacy + civvis::action_space::LEGACY_NUMERIC_WIDTH;
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
        _ => {}
    }
}

/// A stable side for a game, so a rerun trains and validates on the same split.
fn holdout(game: u64, share: f32) -> bool {
    let mut hash = game.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash ^= hash >> 32;
    (hash % 1000) as f32 / 1000.0 < share
}

/// Read complete decision groups from one emitter file. Training and external
/// evaluation share this path so masking, same-kind filtering, and schema
/// checks cannot drift between the two sides of an experiment.
fn load_groups(path: &str, keep: &str, same_kind: bool) -> Result<Loaded, String> {
    let file = fs::File::open(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut head = String::new();
    reader
        .read_line(&mut head)
        .map_err(|error| format!("cannot read header from {path}: {error}"))?;
    let columns = head.trim_end().split(',').count();
    if columns <= 6 + civvis::action_space::FEATURE_WIDTH {
        return Err(format!(
            "{path}: invalid q_dataset schema with {columns} columns"
        ));
    }
    let width = columns - 6; // game,turn,seat,chosen ... won,score_share
    if width < civvis::action_space::FEATURE_WIDTH {
        return Err(format!(
            "{path}: {width} candidate features are narrower than the current action schema {}",
            civvis::action_space::FEATURE_WIDTH
        ));
    }
    let state_block = width - civvis::action_space::FEATURE_WIDTH;
    let kind_count = civvis::action_space::KINDS.len();
    let kind_of = |raw: &[f32]| -> usize {
        (0..kind_count)
            .find(|kind| raw[state_block + kind] > 0.5)
            .unwrap_or(usize::MAX)
    };

    let mut groups = Vec::new();
    let mut current: Option<Group> = None;
    let mut current_kinds: Vec<usize> = Vec::new();
    let mut lines = 0usize;
    let mut dropped = 0usize;
    let finish =
        |group: Group, group_kinds: &[usize], groups: &mut Vec<Group>, dropped: &mut usize| {
            if group.rows.len() < 2 {
                return;
            }
            if same_kind && group_kinds.windows(2).any(|pair| pair[0] != pair[1]) {
                *dropped += 1;
                return;
            }
            groups.push(group);
        };

    for line in reader.lines().map_while(Result::ok) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != columns {
            continue;
        }
        lines += 1;
        let game: u64 = fields[0].parse().unwrap_or(0);
        let chosen = fields[3] == "1";
        let won: f32 = fields[columns - 2].parse().unwrap_or(0.0);
        let mut row: Vec<f32> = fields[4..columns - 2]
            .iter()
            .map(|value| value.parse().unwrap_or(0.0))
            .collect();
        let kind = kind_of(&row);
        mask(&mut row, keep, width);
        if chosen {
            if let Some(group) = current.take() {
                finish(group, &current_kinds, &mut groups, &mut dropped);
            }
            current_kinds = vec![kind];
            current = Some(Group {
                rows: vec![row],
                game,
                won,
                kind,
            });
        } else if let Some(group) = current.as_mut() {
            group.rows.push(row);
            current_kinds.push(kind);
        }
    }
    if let Some(group) = current.take() {
        finish(group, &current_kinds, &mut groups, &mut dropped);
    }
    Ok(Loaded {
        groups,
        columns,
        width,
        lines,
        dropped,
    })
}

struct Net {
    sizes: Vec<usize>,
    weights: Vec<Vec<f32>>, // [layer][in * out]
    biases: Vec<Vec<f32>>,
}

impl Net {
    fn new(width: usize, seed: u64) -> Net {
        let sizes = vec![width, HIDDEN[0], HIDDEN[1], 1];
        let mut state = seed | 1;
        let mut draw = || {
            state = state
                .wrapping_add(0x9E37_79B9_7F4A_7C15)
                .rotate_left(31)
                .wrapping_mul(0xBF58_476D_1CE4_E5B9);
            ((state >> 33) as f32 / (1u64 << 31) as f32) - 0.5
        };
        let mut weights = Vec::new();
        let mut biases = Vec::new();
        for layer in 0..3 {
            let (fan_in, fan_out) = (sizes[layer], sizes[layer + 1]);
            // He-ish scaling: a 124-wide input with unit-ish features saturates
            // a ReLU stack immediately at unit variance.
            let scale = (2.0 / fan_in as f32).sqrt() * 2.0;
            weights.push((0..fan_in * fan_out).map(|_| draw() * scale).collect());
            biases.push(vec![0.0; fan_out]);
        }
        Net {
            sizes,
            weights,
            biases,
        }
    }

    /// Forward pass keeping activations, because the backward pass needs them.
    fn forward(&self, x: &[f32], acts: &mut Vec<Vec<f32>>) -> f32 {
        acts.clear();
        acts.push(x.to_vec());
        for layer in 0..3 {
            let (fan_in, fan_out) = (self.sizes[layer], self.sizes[layer + 1]);
            let input = &acts[layer];
            let mut out = vec![0.0f32; fan_out];
            for o in 0..fan_out {
                let mut sum = self.biases[layer][o];
                for i in 0..fan_in {
                    sum += input[i] * self.weights[layer][i * fan_out + o];
                }
                // Linear on the output layer: this is a ranking score, and a
                // squashed one would flatten the softmax that reads it.
                out[o] = if layer < 2 { sum.max(0.0) } else { sum };
            }
            acts.push(out);
        }
        acts[3][0]
    }

    /// Accumulate d(loss)/d(params) for one row given d(loss)/d(score).
    fn backward(&self, acts: &[Vec<f32>], seed: f32, grad: &mut Grad) {
        let mut delta = vec![seed];
        for layer in (0..3).rev() {
            let (fan_in, fan_out) = (self.sizes[layer], self.sizes[layer + 1]);
            let input = &acts[layer];
            let mut next = vec![0.0f32; fan_in];
            for o in 0..fan_out {
                let d = delta[o];
                if d == 0.0 {
                    continue;
                }
                grad.biases[layer][o] += d;
                for i in 0..fan_in {
                    grad.weights[layer][i * fan_out + o] += d * input[i];
                    next[i] += d * self.weights[layer][i * fan_out + o];
                }
            }
            if layer > 0 {
                for i in 0..fan_in {
                    // ReLU derivative, read off the stored activation.
                    if acts[layer][i] <= 0.0 {
                        next[i] = 0.0;
                    }
                }
            }
            delta = next;
        }
    }
}

struct Grad {
    weights: Vec<Vec<f32>>,
    biases: Vec<Vec<f32>>,
}

impl Grad {
    fn zeros(net: &Net) -> Grad {
        Grad {
            weights: net.weights.iter().map(|w| vec![0.0; w.len()]).collect(),
            biases: net.biases.iter().map(|b| vec![0.0; b.len()]).collect(),
        }
    }
    fn clear(&mut self) {
        for w in &mut self.weights {
            w.iter_mut().for_each(|v| *v = 0.0);
        }
        for b in &mut self.biases {
            b.iter_mut().for_each(|v| *v = 0.0);
        }
    }
}

/// Softmax cross-entropy over one decision's candidates, with the chosen action
/// at index 0. Returns the loss and the credit the argmax earns.
///
/// Credit is tie-aware and that is not a nicety. A head that cannot separate the
/// candidates at all scores every one of them identically, and `max_by` returns
/// the *last* maximum — so a completely blind head reads 0.0% where the honest
/// answer is chance. That is exactly what a state-only ablation does, since
/// every candidate in a decision shares one state vector, and reading it as 0%
/// rather than 20% would have made a structural blindness look like a trained
/// anti-preference. A tie among `k` candidates earns `1/k` when the chosen
/// action is one of them.
fn score_group(net: &Net, group: &Group, grad: Option<&mut Grad>) -> (f32, f32) {
    let mut acts = Vec::new();
    let mut scores = Vec::with_capacity(group.rows.len());
    let mut saved = Vec::with_capacity(group.rows.len());
    for row in &group.rows {
        let score = net.forward(row, &mut acts);
        scores.push(score);
        saved.push(acts.clone());
    }
    let max = scores.iter().cloned().fold(f32::MIN, f32::max);
    let tied = scores.iter().filter(|s| **s >= max).count().max(1);
    let credit = if scores[0] >= max {
        1.0 / tied as f32
    } else {
        0.0
    };
    let exps: Vec<f32> = scores.iter().map(|s| (s - max).exp()).collect();
    let total: f32 = exps.iter().sum::<f32>().max(1e-30);
    let loss = -(exps[0] / total).max(1e-30).ln();
    if let Some(grad) = grad {
        for (index, exp) in exps.iter().enumerate() {
            let probability = exp / total;
            let seed = probability - (index == 0) as u8 as f32;
            net.backward(&saved[index], seed, grad);
        }
    }
    (loss, credit)
}

fn mean_standard_error(values: &[f32]) -> (f32, f32) {
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

/// An aggregate top-1 can be carried by cheap non-tactical decisions. Report
/// each represented kind against its own candidate-count chance rate, so a
/// move-ordering claim has to be true specifically for moves and attacks.
#[derive(Default)]
struct AccuracyTotals {
    hits: f32,
    chance: f32,
    decisions: usize,
}

/// Macro-average a metric over games: decisions inside one game are correlated
/// observations from one trajectory, not independent samples. The top-1 and
/// chance columns therefore give every unseen game equal weight, and the error
/// bar is the standard error across those game-level differences.
fn accuracy_summary(games: &BTreeMap<u64, AccuracyTotals>) -> (usize, f32, f32, f32, f32) {
    let decisions = games.values().map(|totals| totals.decisions).sum();
    let top: Vec<f32> = games
        .values()
        .map(|totals| totals.hits / totals.decisions.max(1) as f32)
        .collect();
    let chance: Vec<f32> = games
        .values()
        .map(|totals| totals.chance / totals.decisions.max(1) as f32)
        .collect();
    let deltas: Vec<f32> = top
        .iter()
        .zip(&chance)
        .map(|(top, chance)| top - chance)
        .collect();
    let (top, _) = mean_standard_error(&top);
    let (chance, _) = mean_standard_error(&chance);
    let (lift, se) = mean_standard_error(&deltas);
    (decisions, chance, top, lift, se)
}

fn report_by_kind(net: &Net, groups: &[Group], minimum: usize, label: &str) {
    let mut all = BTreeMap::<u64, AccuracyTotals>::new();
    let mut by_kind: Vec<BTreeMap<u64, AccuracyTotals>> = (0..civvis::action_space::KINDS.len())
        .map(|_| BTreeMap::new())
        .collect();
    for group in groups {
        if group.kind >= by_kind.len() {
            continue;
        }
        let (_, hit) = score_group(net, group, None);
        let chance = 1.0 / group.rows.len() as f32;
        for totals in [
            all.entry(group.game).or_default(),
            by_kind[group.kind].entry(group.game).or_default(),
        ] {
            totals.hits += hit;
            totals.chance += chance;
            totals.decisions += 1;
        }
    }

    let mut order: Vec<usize> = (0..by_kind.len())
        .filter(|kind| {
            by_kind[*kind]
                .values()
                .map(|totals| totals.decisions)
                .sum::<usize>()
                >= minimum
        })
        .collect();
    order.sort_by(|left, right| {
        let decisions = |kind: usize| {
            by_kind[kind]
                .values()
                .map(|totals| totals.decisions)
                .sum::<usize>()
        };
        decisions(*right).cmp(&decisions(*left)).then_with(|| {
            civvis::action_space::KINDS[*left].cmp(civvis::action_space::KINDS[*right])
        })
    });
    println!("{label} by chosen action kind (game-macro, minimum {minimum} decisions):");
    println!("  kind                    decisions games  chance   top-1        lift");
    let print = |name: &str, games: &BTreeMap<u64, AccuracyTotals>| {
        let (decisions, chance, top, lift, se) = accuracy_summary(games);
        println!(
            "  {name:<23} {decisions:>9} {:>5}  {:>5.1}%  {:>6.1}%  {:+6.1} +/- {:>4.1} pp",
            games.len(),
            100.0 * chance,
            100.0 * top,
            100.0 * lift,
            100.0 * se,
        );
    };
    print("all", &all);
    for kind in order {
        print(civvis::action_space::KINDS[kind], &by_kind[kind]);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = text(&args, "--data", "evolved/q_dataset.csv");
    let eval_data = args
        .iter()
        .position(|arg| arg == "--eval-data")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let out = text(&args, "--out", "evolved/qnet.json");
    let epochs = number(&args, "--epochs", 4);
    let batch = number(&args, "--batch", 64);
    let rate = decimal(&args, "--rate", 0.02);
    let share = decimal(&args, "--holdout", 0.25);
    let cap = number(&args, "--max-groups", 120_000);
    let kind_min = number(&args, "--kind-min", 100);
    // Which blocks of the row the head may see. `legacy-geometry` preserves
    // the thirteen terms that failed the same-actor external gate;
    // `destination` isolates the new terrain, force-field, role, and explicit
    // plan-progress block. `geometry` exposes both. On same-actor groups the
    // kind and actor context are constant, so only destination differences can
    // move the ranking.
    let keep = text(&args, "--keep", "all");
    // Keep only decisions whose candidates are all the same kind of action.
    //
    // This is the control that decides whether the head is useful. A kind
    // one-hot alone reaches the same top-1 as the full vector, so most of what
    // is being learned is "the expert moves more often than it fortifies" —
    // true, and useless for choosing *which* move. On same-kind decisions the
    // kind one-hot is constant across every candidate, so it can only tie, and
    // any lift over chance is the geometry genuinely discriminating siblings.
    // That is the signal a search prior needs and the coarse prior cannot give.
    let same_kind = args.iter().any(|arg| arg == "--same-kind");

    let loaded = match load_groups(&data, &keep, same_kind) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let width = loaded.width;
    println!(
        "{data}: {} columns, {width} features per candidate, keeping {keep}",
        loaded.columns
    );
    if same_kind {
        println!(
            "--same-kind: dropped {} mixed-kind decisions",
            loaded.dropped
        );
    }

    let source_lines = loaded.lines;
    let (train, valid, external) = if let Some(path) = eval_data.as_deref() {
        let evaluated = match load_groups(path, &keep, same_kind) {
            Ok(loaded) => loaded,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
        if evaluated.width != width {
            eprintln!(
                "q_train: training width {width} does not match external evaluation width {}",
                evaluated.width
            );
            std::process::exit(1);
        }
        println!(
            "external evaluation {path}: {} groups from {} rows{}",
            evaluated.groups.len(),
            evaluated.lines,
            if same_kind {
                format!(", dropped {} mixed-kind decisions", evaluated.dropped)
            } else {
                String::new()
            }
        );
        (
            loaded.groups.into_iter().take(cap).collect::<Vec<_>>(),
            evaluated.groups,
            true,
        )
    } else {
        let mut train = Vec::new();
        let mut valid = Vec::new();
        for group in loaded.groups {
            if holdout(group.game, share) {
                valid.push(group);
            } else if train.len() < cap {
                train.push(group);
            }
        }
        (train, valid, false)
    };

    let mean_candidates =
        train.iter().map(|g| g.rows.len()).sum::<usize>() as f32 / train.len().max(1) as f32;
    let evaluation_label = if external { "external" } else { "held-out" };
    let train_chance = train
        .iter()
        .map(|group| 1.0 / group.rows.len() as f32)
        .sum::<f32>()
        / train.len().max(1) as f32;
    let valid_chance = valid
        .iter()
        .map(|group| 1.0 / group.rows.len() as f32)
        .sum::<f32>()
        / valid.len().max(1) as f32;
    println!(
        "{source_lines} source rows -> {} train decisions, {} {} decisions, {mean_candidates:.2} train candidates each",
        train.len(),
        valid.len(),
        if external {
            "external-evaluation"
        } else {
            "held-out"
        },
    );
    println!(
        "exact chance top-1: train {:.1}%, {} {:.1}% (one chosen action among {mean_candidates:.2} mean train candidates)",
        100.0 * train_chance,
        evaluation_label,
        100.0 * valid_chance,
    );
    if train.is_empty() || valid.is_empty() {
        eprintln!("q_train: need both training and evaluation decisions");
        std::process::exit(1);
    }
    let won_share = train.iter().filter(|g| g.won > 0.5).count() as f32 / train.len() as f32;
    println!(
        "train decisions from winning seats: {:.1}%",
        100.0 * won_share
    );

    let mut net = Net::new(width, 12_345);
    let mut grad = Grad::zeros(&net);
    let mut order: Vec<usize> = (0..train.len()).collect();
    let mut shuffle = 0x243F_6A88_85A3_08D3u64;

    for epoch in 1..=epochs {
        // Fisher-Yates off a fixed stream, so an epoch is reproducible.
        for index in (1..order.len()).rev() {
            shuffle = shuffle
                .wrapping_add(0x9E37_79B9_7F4A_7C15)
                .rotate_left(31)
                .wrapping_mul(0xBF58_476D_1CE4_E5B9);
            order.swap(index, (shuffle >> 33) as usize % (index + 1));
        }
        let mut seen = 0usize;
        let mut loss_sum = 0.0f32;
        let mut hits = 0.0f32;
        grad.clear();
        for (step, index) in order.iter().enumerate() {
            let (loss, hit) = score_group(&net, &train[*index], Some(&mut grad));
            loss_sum += loss;
            hits += hit;
            seen += 1;
            if (step + 1) % batch == 0 || step + 1 == order.len() {
                let scale = rate / batch as f32;
                for layer in 0..3 {
                    for (weight, g) in net.weights[layer].iter_mut().zip(&grad.weights[layer]) {
                        *weight -= scale * g;
                    }
                    for (bias, g) in net.biases[layer].iter_mut().zip(&grad.biases[layer]) {
                        *bias -= scale * g;
                    }
                }
                grad.clear();
            }
        }
        let (vloss, vhits) = valid.iter().fold((0.0f32, 0.0f32), |(l, h), group| {
            let (loss, hit) = score_group(&net, group, None);
            (l + loss, h + hit)
        });
        println!(
            "epoch {epoch}: train loss {:.4} top-1 {:.1}% | {evaluation_label} loss {:.4} top-1 {:.1}%",
            loss_sum / seen as f32,
            100.0 * hits / seen as f32,
            vloss / valid.len() as f32,
            100.0 * vhits / valid.len() as f32
        );
    }
    report_by_kind(&net, &valid, kind_min, evaluation_label);

    // Written in the `ValueNet` shape so the existing loader reads it: the
    // hidden stack is pinned at [64, 32, 1] and only the input width is free.
    let mut json = String::from("{\"sizes\":[");
    json.push_str(
        &net.sizes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    json.push_str("],\"weights\":[");
    for (layer, weights) in net.weights.iter().enumerate() {
        let (fan_in, fan_out) = (net.sizes[layer], net.sizes[layer + 1]);
        if layer > 0 {
            json.push(',');
        }
        json.push('[');
        for i in 0..fan_in {
            if i > 0 {
                json.push(',');
            }
            json.push('[');
            for o in 0..fan_out {
                if o > 0 {
                    json.push(',');
                }
                json.push_str(&format!("{:.6}", weights[i * fan_out + o]));
            }
            json.push(']');
        }
        json.push(']');
    }
    json.push_str("],\"biases\":[");
    for (layer, biases) in net.biases.iter().enumerate() {
        if layer > 0 {
            json.push(',');
        }
        json.push('[');
        json.push_str(
            &biases
                .iter()
                .map(|b| format!("{b:.6}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        json.push(']');
    }
    json.push_str("]}");

    if let Some(parent) = std::path::Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    match fs::File::create(&out).and_then(|mut f| f.write_all(json.as_bytes())) {
        Ok(()) => println!("wrote {out}"),
        Err(error) => eprintln!("cannot write {out}: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{accuracy_summary, mask, AccuracyTotals};
    use std::collections::BTreeMap;

    /// A game with nine decisions must not outweigh a game with one decision.
    /// The model is evaluated on unseen trajectories, so games are the
    /// independent units and the uncertainty must be clustered at that level.
    #[test]
    fn accuracy_is_macro_averaged_by_game() {
        let games = BTreeMap::from([
            (
                1,
                AccuracyTotals {
                    hits: 1.0,
                    chance: 0.5,
                    decisions: 1,
                },
            ),
            (
                2,
                AccuracyTotals {
                    hits: 0.0,
                    chance: 4.5,
                    decisions: 9,
                },
            ),
        ]);
        let (decisions, chance, top, lift, se) = accuracy_summary(&games);
        assert_eq!(decisions, 10);
        assert!((chance - 0.5).abs() < 1e-6);
        assert!((top - 0.5).abs() < 1e-6);
        assert!(lift.abs() < 1e-6);
        assert!((se - 0.5).abs() < 1e-6);
    }

    #[test]
    fn feature_ablation_separates_legacy_from_destination_geometry() {
        let state = civvis::decision_features::WIDTH;
        let kinds = civvis::action_space::KINDS.len();
        let legacy = civvis::action_space::LEGACY_NUMERIC_WIDTH;
        let width = state + civvis::action_space::FEATURE_WIDTH;
        let destination = state + kinds + legacy;

        let mut only_destination = vec![1.0; width];
        mask(&mut only_destination, "destination", width);
        assert!(only_destination[..destination].iter().all(|value| *value == 0.0));
        assert!(only_destination[destination..].iter().all(|value| *value == 1.0));

        let mut only_legacy = vec![1.0; width];
        mask(&mut only_legacy, "legacy-geometry", width);
        assert!(only_legacy[..state + kinds]
            .iter()
            .all(|value| *value == 0.0));
        assert!(only_legacy[state + kinds..destination]
            .iter()
            .all(|value| *value == 1.0));
        assert!(only_legacy[destination..].iter().all(|value| *value == 0.0));
    }
}
