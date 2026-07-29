//! Can this encoding represent the expert's policy at all?
//!
//! Before anything is wired into an agent, one cheap question gates the whole
//! action-conditioned programme: given a state and the set of actions that were
//! legal in it, can a head trained on `q_dataset` rows pick the action
//! `AdvancedAi` actually took? With `--negatives 4` each decision is one chosen
//! action against four that were not, so **20% is chance**. A head that cannot
//! beat 20% on held-out games has shown that the 124-dimensional encoding does
//! not carry the expert's decision, and no downstream use of it — prior, Q,
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
//! ```
//!
//! **Splitting is by game and nothing else.** A per-sample split of the previous
//! value-net data read 98.8% where a per-game split read 75.0%. Rows inside one
//! game share an outcome, a map and an opponent set; any split that mixes them
//! measures memorisation. Games are assigned by hashing the game id, so the same
//! game lands on the same side every run.
//!
//! **Groups are runs, by the emitter's contract.** `q_dataset` writes a chosen
//! row and then its negatives, so a group begins at each `chosen=1`. That is
//! pinned by a test over there rather than assumed here.
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
}

/// Blank the blocks this run is not allowed to see. `state` is the leading 34,
/// then the action block: its first 77 are the kind one-hot and the remaining 13
/// are the geometry (target tile, HP, distance, treasury, plot cost).
fn mask(row: &mut [f32], keep: &str, width: usize) {
    let state = width - civvis::action_space::FEATURE_WIDTH;
    let kinds = civvis::action_space::KINDS.len();
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = text(&args, "--data", "evolved/q_dataset.csv");
    let out = text(&args, "--out", "evolved/qnet.json");
    let epochs = number(&args, "--epochs", 4);
    let batch = number(&args, "--batch", 64);
    let rate = decimal(&args, "--rate", 0.02);
    let share = decimal(&args, "--holdout", 0.25);
    let cap = number(&args, "--max-groups", 120_000);
    // Which blocks of the row the head may see. The control that matters is
    // `kind`: 77 of the 90 action features are a kind one-hot and the expert's
    // kind distribution is skewed, so a head that learned nothing but "pick a
    // move" would still score well above chance. If `kind` reaches the same
    // top-1 as `all`, the state and the geometry are carrying nothing and the
    // ranker is a kind prior in a costume.
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

    let file = match fs::File::open(&data) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("cannot read {data}: {error}");
            std::process::exit(1);
        }
    };
    let mut reader = BufReader::new(file);
    let mut head = String::new();
    let _ = reader.read_line(&mut head);
    let columns = head.trim_end().split(',').count();
    let width = columns - 6; // game,turn,seat,chosen ... won,score_share
    println!("{data}: {columns} columns, {width} features per candidate, keeping {keep}");

    let mut train: Vec<Group> = Vec::new();
    let mut valid: Vec<Group> = Vec::new();
    let mut current: Option<Group> = None;
    let mut lines = 0usize;
    let state_block = width - civvis::action_space::FEATURE_WIDTH;
    let kind_count = civvis::action_space::KINDS.len();
    // Read off the raw row, before `mask` blanks anything.
    let kind_of = |raw: &[f32]| -> usize {
        (0..kind_count)
            .find(|k| raw[state_block + k] > 0.5)
            .unwrap_or(usize::MAX)
    };
    let mut current_kinds: Vec<usize> = Vec::new();
    let mut dropped = 0usize;

    // A finished group is filed here so the two call sites (mid-loop and the
    // tail) cannot drift apart on the filter or the split.
    let mut file = |group: Group, group_kinds: &[usize], train: &mut Vec<Group>, valid: &mut Vec<Group>, dropped: &mut usize| {
        if group.rows.len() < 2 {
            return;
        }
        if same_kind && group_kinds.windows(2).any(|pair| pair[0] != pair[1]) {
            *dropped += 1;
            return;
        }
        if holdout(group.game, share) {
            valid.push(group);
        } else if train.len() < cap {
            train.push(group);
        }
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
        mask(&mut row, &keep, width);
        if chosen {
            if let Some(group) = current.take() {
                file(group, &current_kinds, &mut train, &mut valid, &mut dropped);
            }
            current_kinds = vec![kind];
            current = Some(Group {
                rows: vec![row],
                game,
                won,
            });
        } else if let Some(group) = current.as_mut() {
            group.rows.push(row);
            current_kinds.push(kind);
        }
    }
    if let Some(group) = current.take() {
        file(group, &current_kinds, &mut train, &mut valid, &mut dropped);
    }
    if same_kind {
        println!("--same-kind: dropped {dropped} mixed-kind decisions");
    }

    let mean_candidates = train.iter().map(|g| g.rows.len()).sum::<usize>() as f32
        / train.len().max(1) as f32;
    println!(
        "{lines} rows -> {} train decisions, {} held-out decisions, {mean_candidates:.2} candidates each",
        train.len(),
        valid.len()
    );
    println!(
        "chance top-1 = {:.1}% (one chosen action among {mean_candidates:.2})",
        100.0 / mean_candidates
    );
    if train.is_empty() || valid.is_empty() {
        eprintln!("q_train: need decisions on both sides of the split");
        std::process::exit(1);
    }
    let won_share = train.iter().filter(|g| g.won > 0.5).count() as f32 / train.len() as f32;
    println!("train decisions from winning seats: {:.1}%", 100.0 * won_share);

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
            "epoch {epoch}: train loss {:.4} top-1 {:.1}% | held-out loss {:.4} top-1 {:.1}%",
            loss_sum / seen as f32,
            100.0 * hits / seen as f32,
            vloss / valid.len() as f32,
            100.0 * vhits / valid.len() as f32
        );
    }

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
