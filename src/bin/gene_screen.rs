//! Gene screen: price EVERY treatment flag from ONE batch of random-genome games.
//!
//! Every behaviour the live bridge and production turn on is a boolean flag
//! with a named withholding twin (`LIVE_TREATMENTS`, `PRODUCTION_TREATMENTS`,
//! `PRODUCTION_OPT_INS`). Read as genes, each is on or off, and the question
//! the whole evaluation lane keeps asking is the same one for each of them:
//! *does the agent win more with this gene on than off?*
//!
//! The existing answer is one arm per gene — `live` against
//! `live_without_<flag>`, forty to two hundred maps each — and it has two
//! costs. It is priced per gene, so pricing fifty-seven genes at two hundred
//! maps is eleven thousand games. And it measures each repair against a
//! background in which every OTHER repair is on, which
//! `AdvancedAi::enable_engine_repairs` itself warns is a link priced inside an
//! otherwise-whole chain.
//!
//! This binary runs the classical screening design instead. Every game seats
//! ONE treated major whose genome is drawn at random — each screened gene on
//! or off with probability one half — against a stock field, and records the
//! genome beside the outcome. Games come in **foldover pairs**: the second
//! game of a pair replays the SAME map seed and seat with the COMPLEMENT
//! genome, so every gene is on in exactly one arm of every pair and the map's
//! own difficulty cancels out of every per-gene difference. Every game then
//! informs every gene: `N` pairs give each gene `N` games on and `N` off, and
//! the per-gene effect is the mean paired difference in the outcome, averaged
//! over random backgrounds rather than over the all-on one.
//!
//! What it prices per gene, with intervals:
//!
//! - win rate on vs off (the treated seat winning by any victory), Δ in points
//!   with a 95% CI and z from the paired differences;
//! - the same for **score share** (treated score over all majors' scores), a
//!   continuous outcome that resolves an edge at a fraction of the games a
//!   win/loss count needs;
//! - an OLS-adjusted Δ that regresses the paired difference on the whole
//!   ±1 sign matrix at once, so a gene is not credited with the chance
//!   imbalance of its neighbours (printed once the pair count can support it).
//!
//! ⚠ It is a SCREEN. Fifty-seven genes at |z| ≥ 2 flag ~2.6 of them by chance
//! alone; the table prints that number, the family-wise |z| bar, and the
//! smallest Δ the run could resolve at 80% power, so a `~` row is read as
//! "unresolved at this size" and never as "no effect". Interactions are not
//! estimated here — the per-game rows are written to a JSONL file precisely so
//! a later pass (epistasis, subgroup by map, a fitted logistic) never has to
//! replay a game. `--analyze` recomputes the table from those rows and merges
//! several runs' files.
//!
//! ⚠ The genome carries the NATIVE bundle only: `ENGINE_REPAIR_TREATMENTS`
//! (the live bridge minus `FIRAXIS_ONLY_TREATMENTS`, which read host-only
//! state and are inert on a CIVVIS board), plus the production treatments and
//! opt-ins. A Firaxis-only flag screened here would measure noise and be
//! reported as noise; it is excluded rather than measured.
//!
//! This is NOT `gene_census`, which asks whether a continuous `Weights` gene
//! moves an outcome at all. The genes here are the boolean treatment flags.
//!
//! Usage:
//!   gene_screen [--pairs N] [--start-seed N] [--players N] [--turns N]
//!               [--width N] [--height N] [--city-states N] [--speed ID]
//!               [--map ID] [--jobs N] [--genes tag,tag,...]
//!               [--baseline repairs|stock] [--field advanced|repairs]
//!               [--anchor-pairs N] [--randomize-civs] [--out PATH] [--append]
//!               [--quiet]
//!   gene_screen --analyze PATH [PATH ...]
//!   gene_screen --list
//!
//! Defaults play 4 majors on 60x38 Pangaea at Online speed to its own 250-turn
//! clock. `--players 6 --width 74 --height 46 --city-states 9` is the
//! deployment shape (`docs/EVAL.md`); quote no number without its profile.
use civvis::ai::{run_game, AdvancedAi, LiveTreatment};
use civvis::game::{Game, GameOptions};
use civvis::rng::Rng;
use civvis::setup::{GameSpeed, MapScript};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::time::Instant;

/// One boolean treatment flag read as a gene.
///
/// `after_setup_on` is the flag's state after the treated seat is built (stock
/// production plus `enable_engine_repairs`), `stock_on` its state on the
/// production agent alone, and `flip` the toggle that moves it away from
/// `after_setup_on`.
#[derive(Clone, Copy)]
struct Gene {
    field: &'static str,
    tag: &'static str,
    after_setup_on: bool,
    stock_on: bool,
    flip: fn(&mut AdvancedAi),
}

/// Every gene this screen can vary, in the order the genome bits are written.
///
/// ⚠ Discovered from the repository's own tables, never listed by hand: a
/// treatment added to `ENGINE_REPAIR_TREATMENTS`, `PRODUCTION_TREATMENTS` or
/// `PRODUCTION_OPT_INS` reaches the genome without touching this file. An
/// engine-repair tag with no `LIVE_TREATMENTS` row is a panic, not a silent
/// omission — the elo tests already hold the two tables in step and this
/// binary trusts that contract loudly.
fn gene_table() -> Vec<Gene> {
    let mut genes = Vec::new();
    for repair in civvis::elo::ENGINE_REPAIR_TREATMENTS {
        let &(field, tag, disable): &LiveTreatment = civvis::ai::LIVE_TREATMENTS
            .iter()
            .find(|(_, row_tag, _)| row_tag == repair)
            .unwrap_or_else(|| {
                panic!(
                    "engine repair {repair} has no LIVE_TREATMENTS row, so it cannot be withheld"
                )
            });
        genes.push(Gene {
            field,
            tag,
            after_setup_on: true,
            stock_on: false,
            flip: disable,
        });
    }
    for &(field, tag, disable) in civvis::ai::PRODUCTION_TREATMENTS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: true,
            stock_on: true,
            flip: disable,
        });
    }
    for &(field, tag, enable) in civvis::ai::PRODUCTION_OPT_INS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: false,
            stock_on: false,
            flip: enable,
        });
    }
    genes
}

/// What the un-screened genes are held at.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Baseline {
    /// The native repair bundle (`advanced_synergy`): every engine repair on.
    Repairs,
    /// Production `advanced`: every engine repair off.
    Stock,
}

/// Who the treated seat plays against.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Field {
    /// Production `advanced`, the ladder's incumbent.
    Advanced,
    /// The native repair bundle.
    Repairs,
}

/// One screened game's row, written to the JSONL file and read back by
/// `--analyze`.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Row {
    /// `game` for a screened pair member, `anchor` for an all-on/all-off pair.
    kind: String,
    pair: usize,
    /// 0 = the drawn genome, 1 = its complement (anchor: 0 = all on, 1 = all off).
    arm: u8,
    seed: u64,
    seat: usize,
    /// One char per gene in header order: `1` on, `0` off.
    genome: String,
    win: bool,
    winner: Option<usize>,
    victory: String,
    turn: u32,
    score: i64,
    /// Treated score over the sum of every major's score.
    score_share: f64,
    /// 1 = highest score among majors.
    rank: usize,
    cities: usize,
    alive: bool,
    secs: f64,
}

/// The first line of the JSONL file: the gene order every genome string is
/// written in, and the profile the games were played at.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Header {
    kind: String,
    genes: Vec<String>,
    screened: Vec<String>,
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    speed: String,
    map: String,
    baseline: String,
    field: String,
    start_seed: u64,
    /// Whether every seat's civilization was shuffled per map instead of the
    /// stock order (Rome, Egypt, Greece, China, … by seat). Absent in files
    /// written before the flag existed, which means `false`.
    #[serde(default)]
    randomize_civs: bool,
}

fn number(args: &[String], flag: &str, default: i64) -> i64 {
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

fn present(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

/// Draw one genome: each screened gene on with probability one half, seeded
/// from the screen's start seed and the pair index so a run reproduces
/// exactly and two runs on disjoint seed windows draw disjoint genomes.
fn draw_genome(start_seed: u64, pair: usize, screened: &[bool]) -> Vec<bool> {
    let mut rng = Rng::new(
        start_seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(pair as u64)
            .wrapping_add(0x5EED_6E4E),
    );
    screened
        .iter()
        .map(|&is_screened| is_screened && rng.chance(0.5))
        .collect()
}

/// The foldover: every screened gene flipped, un-screened genes untouched.
fn complement(genome: &[bool], screened: &[bool]) -> Vec<bool> {
    genome
        .iter()
        .zip(screened)
        .map(|(&on, &is_screened)| if is_screened { !on } else { on })
        .collect()
}

fn genome_string(genome: &[bool]) -> String {
    genome
        .iter()
        .map(|&on| if on { '1' } else { '0' })
        .collect()
}

/// Build the treated seat: production plus the repair bundle, then every gene
/// set to its desired state — the genome bit when screened, the baseline
/// otherwise.
fn treated_seat(
    genes: &[Gene],
    genome: &[bool],
    screened: &[bool],
    baseline: Baseline,
) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    for ((gene, &on), &is_screened) in genes.iter().zip(genome).zip(screened) {
        let desired = if is_screened {
            on
        } else {
            match baseline {
                Baseline::Repairs => gene.after_setup_on,
                Baseline::Stock => gene.stock_on,
            }
        };
        if desired != gene.after_setup_on {
            (gene.flip)(&mut ai);
        }
    }
    ai
}

fn field_seat(field: Field) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if field == Field::Repairs {
        ai.enable_engine_repairs();
    }
    ai
}

struct Profile {
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    speed: GameSpeed,
    map: MapScript,
    randomize_civs: bool,
}

/// Play one game with the treated seat carrying `genome` and report its row.
#[allow(clippy::too_many_arguments)]
fn play(
    profile: &Profile,
    genes: &[Gene],
    screened: &[bool],
    baseline: Baseline,
    field: Field,
    kind: &str,
    pair: usize,
    arm: u8,
    seed: u64,
    seat: usize,
    genome: &[bool],
) -> Row {
    let started = Instant::now();
    let mut game = Game::new_with(GameOptions {
        speed: profile.speed.id().to_string(),
        map_script: profile.map,
        randomize_civs: profile.randomize_civs,
        ..GameOptions::new(
            profile.players,
            profile.width,
            profile.height,
            seed,
            profile.turns,
            profile.city_states,
        )
    });
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|pid| {
            if pid == seat {
                treated_seat(genes, genome, screened, baseline)
            } else if game.players[pid].is_minor || game.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                field_seat(field)
            }
        })
        .collect();
    run_game(&mut game, &mut ais);

    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    let scores: BTreeMap<usize, i64> = majors.iter().map(|&pid| (pid, game.score(pid))).collect();
    let total: i64 = scores.values().sum();
    let score = scores.get(&seat).copied().unwrap_or(0);
    let rank = 1 + scores.values().filter(|&&other| other > score).count();
    Row {
        kind: kind.to_string(),
        pair,
        arm,
        seed,
        seat,
        genome: genome_string(genome),
        win: game.winner == Some(seat),
        winner: game.winner,
        victory: game.victory_type.clone().unwrap_or_default(),
        turn: game.reported_turn(),
        score,
        score_share: if total > 0 {
            score as f64 / total as f64
        } else {
            0.0
        },
        rank,
        cities: game.player_city_ids(seat).len(),
        alive: game.players[seat].alive,
        secs: started.elapsed().as_secs_f64(),
    }
}

// ----------------------------------------------------------------- statistics

/// Standard normal CDF (Abramowitz & Stegun 7.1.26, |error| < 1.5e-7).
fn normal_cdf(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * z.abs() / std::f64::consts::SQRT_2);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erf = 1.0 - poly * (-(z * z) / 2.0).exp();
    0.5 * (1.0 + if z >= 0.0 { erf } else { -erf })
}

/// Upper-tail quantile: the `z` with `P(Z > z) = p`, by bisection.
fn normal_quantile_upper(p: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 40.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if 1.0 - normal_cdf(mid) > p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Mean and standard error of a sample of paired differences.
fn mean_se(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if values.is_empty() {
        return (0.0, f64::INFINITY);
    }
    let mean = values.iter().sum::<f64>() / n;
    if values.len() < 2 {
        return (mean, f64::INFINITY);
    }
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// Solve `A x = b` in place by Gaussian elimination with partial pivoting.
/// Returns `None` when the matrix is singular to working precision.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            if factor == 0.0 {
                continue;
            }
            let pivot_row = a[col].clone();
            for (entry, pivot_entry) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *entry -= factor * pivot_entry;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for k in row + 1..n {
            sum -= a[row][k] * x[k];
        }
        x[row] = sum / a[row][row];
    }
    Some(x)
}

/// Invert a symmetric positive matrix column by column; `None` if singular.
fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut columns = Vec::with_capacity(n);
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        columns.push(solve(a.to_vec(), e)?);
    }
    // columns[j][i] is entry (i, j) of the inverse.
    Some(
        (0..n)
            .map(|i| (0..n).map(|j| columns[j][i]).collect())
            .collect(),
    )
}

/// OLS of the paired differences on the ±1 sign matrix, no intercept: the
/// coefficient on gene `i` is its on-vs-off effect adjusted for every other
/// gene's chance imbalance across the pairs. Returns (effect, se) per gene, or
/// `None` when the design cannot support it.
fn adjusted_effects(signs: &[Vec<f64>], diffs: &[f64]) -> Option<Vec<(f64, f64)>> {
    let n = diffs.len();
    let k = signs.first()?.len();
    if k == 0 || n < 2 * k + 10 {
        return None;
    }
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (row, &d) in signs.iter().zip(diffs) {
        for i in 0..k {
            xty[i] += row[i] * d;
            for j in 0..k {
                xtx[i][j] += row[i] * row[j];
            }
        }
    }
    let inverse = invert(&xtx)?;
    let beta: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| inverse[i][j] * xty[j]).sum())
        .collect();
    let rss: f64 = signs
        .iter()
        .zip(diffs)
        .map(|(row, &d)| {
            let fitted: f64 = row.iter().zip(&beta).map(|(s, b)| s * b).sum();
            (d - fitted).powi(2)
        })
        .sum();
    let sigma2 = rss / (n - k) as f64;
    Some(
        (0..k)
            .map(|i| (beta[i], (sigma2 * inverse[i][i]).max(0.0).sqrt()))
            .collect(),
    )
}

/// One gene's estimates from the pairs.
#[derive(Clone, Debug)]
struct GeneEstimate {
    tag: String,
    pairs: usize,
    win_on: f64,
    win_off: f64,
    /// Win-rate Δ (on − off) with its standard error.
    win_delta: f64,
    win_se: f64,
    share_delta: f64,
    share_se: f64,
    adjusted: Option<(f64, f64)>,
}

impl GeneEstimate {
    fn win_z(&self) -> f64 {
        if self.win_se > 0.0 && self.win_se.is_finite() {
            self.win_delta / self.win_se
        } else {
            0.0
        }
    }
    fn share_z(&self) -> f64 {
        if self.share_se > 0.0 && self.share_se.is_finite() {
            self.share_delta / self.share_se
        } else {
            0.0
        }
    }
}

/// Group `game` rows into complete pairs and estimate every gene.
///
/// A pair is complete when both arms are present for one `(seed, seat, pair)`
/// key; an unfinished run's odd row is dropped rather than counted as an
/// unpaired game. Merged files may repeat a key only if they replayed the same
/// pair, in which case the later row wins.
fn estimate(header: &Header, rows: &[Row]) -> (Vec<GeneEstimate>, usize, f64, f64) {
    let k = header.genes.len();
    let mut pairs: BTreeMap<(u64, usize, usize), [Option<&Row>; 2]> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.kind == "game") {
        let slot = pairs
            .entry((row.seed, row.seat, row.pair))
            .or_insert([None, None]);
        slot[usize::from(row.arm.min(1))] = Some(row);
    }
    let complete: Vec<(&Row, &Row)> = pairs
        .values()
        .filter_map(|[a, b]| Some(((*a)?, (*b)?)))
        .collect();
    let treated_wins = complete
        .iter()
        .map(|(a, b)| usize::from(a.win) + usize::from(b.win))
        .sum::<usize>();
    let treated_share = complete
        .iter()
        .map(|(a, b)| a.score_share + b.score_share)
        .sum::<f64>();
    let games = complete.len() * 2;
    let overall_win = if games > 0 {
        treated_wins as f64 / games as f64
    } else {
        0.0
    };
    let overall_share = if games > 0 {
        treated_share / games as f64
    } else {
        0.0
    };

    let screened: Vec<bool> = header
        .genes
        .iter()
        .map(|gene| header.screened.contains(gene))
        .collect();
    let mut signs: Vec<Vec<f64>> = Vec::with_capacity(complete.len());
    let mut win_diffs: Vec<f64> = Vec::with_capacity(complete.len());
    let mut share_diffs: Vec<f64> = Vec::with_capacity(complete.len());
    for (a, b) in &complete {
        let bits_a: Vec<bool> = a.genome.chars().map(|c| c == '1').collect();
        let bits_b: Vec<bool> = b.genome.chars().map(|c| c == '1').collect();
        if bits_a.len() != k || bits_b.len() != k {
            continue;
        }
        signs.push(
            (0..k)
                .filter(|&i| screened[i])
                .map(|i| if bits_a[i] { 1.0 } else { -1.0 })
                .collect(),
        );
        win_diffs.push(f64::from(u8::from(a.win)) - f64::from(u8::from(b.win)));
        share_diffs.push(a.score_share - b.score_share);
    }
    let adjusted = adjusted_effects(&signs, &win_diffs);

    let mut estimates = Vec::new();
    let mut screened_index = 0;
    for (i, tag) in header.genes.iter().enumerate() {
        if !screened[i] {
            continue;
        }
        let column = screened_index;
        screened_index += 1;
        // Orient every pair so the difference reads on − off for this gene.
        let oriented_win: Vec<f64> = signs
            .iter()
            .zip(&win_diffs)
            .map(|(row, d)| row[column] * d)
            .collect();
        let oriented_share: Vec<f64> = signs
            .iter()
            .zip(&share_diffs)
            .map(|(row, d)| row[column] * d)
            .collect();
        let (win_delta, win_se) = mean_se(&oriented_win);
        let (share_delta, share_se) = mean_se(&oriented_share);
        // Win rate on/off from the same pairs: each pair contributes exactly
        // one on-arm and one off-arm.
        let mut wins_on = 0usize;
        let mut wins_off = 0usize;
        for ((a, b), row) in complete.iter().zip(&signs) {
            let (on, off) = if row[column] > 0.0 { (a, b) } else { (b, a) };
            wins_on += usize::from(on.win);
            wins_off += usize::from(off.win);
        }
        let n = signs.len();
        estimates.push(GeneEstimate {
            tag: tag.clone(),
            pairs: n,
            win_on: if n > 0 {
                wins_on as f64 / n as f64
            } else {
                0.0
            },
            win_off: if n > 0 {
                wins_off as f64 / n as f64
            } else {
                0.0
            },
            win_delta,
            win_se,
            share_delta,
            share_se,
            adjusted: adjusted.as_ref().map(|all| all[column]),
        });
    }
    (estimates, complete.len(), overall_win, overall_share)
}

/// The `read` column: the win-Δ verdict, then the score-share verdict when it
/// says more. Share resolves an edge at a fraction of the games a win count
/// needs, so a gene the win column cannot yet see is often already loud here
/// — and a reader sorting by the win z would otherwise never meet it.
fn read_column(win_z: f64, share_z: f64, family_z: f64) -> String {
    let word = |z: f64| -> Option<&'static str> {
        if z.abs() >= family_z {
            Some(if z > 0.0 { "HELPS **" } else { "HURTS **" })
        } else if z.abs() >= 2.0 {
            Some(if z > 0.0 { "helps *" } else { "hurts *" })
        } else {
            None
        }
    };
    match (word(win_z), word(share_z)) {
        (None, None) => "~".to_string(),
        (Some(win), None) => win.to_string(),
        (None, Some(share)) => format!("share {share}"),
        (Some(win), Some(share)) => format!("{win} · share {share}"),
    }
}

fn print_table(header: &Header, rows: &[Row]) {
    let (mut estimates, pairs, overall_win, overall_share) = estimate(header, rows);
    let anchors: Vec<&Row> = rows.iter().filter(|row| row.kind == "anchor").collect();
    println!(
        "\ngene screen · {} complete pairs ({} games) · {}p {}x{} {} · {} · {} turns · {} city-states · baseline {} · field {} · {}",
        pairs,
        pairs * 2,
        header.players,
        header.width,
        header.height,
        header.map,
        header.speed,
        header.turns,
        header.city_states,
        header.baseline,
        header.field,
        if header.randomize_civs {
            "shuffled civs"
        } else {
            "stock-seated civs"
        }
    );
    println!(
        "treated seat overall: win {:.1}% (chance {:.1}%) · score share {:.1}% (equal share {:.1}%)",
        100.0 * overall_win,
        100.0 / header.players as f64,
        100.0 * overall_share,
        100.0 / header.players as f64
    );
    {
        // The regime, so a table is never read without knowing what decided
        // its games: two thirds of native 4p games end by conversion before
        // a siege can matter, and that is visible only here.
        let screened_rows: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
        let mut census: BTreeMap<&str, (usize, Vec<u32>)> = BTreeMap::new();
        for row in &screened_rows {
            let entry = census
                .entry(row.victory.as_str())
                .or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(row.turn);
        }
        let mut kinds: Vec<_> = census.into_iter().collect();
        kinds.sort_by_key(|kind| std::cmp::Reverse(kind.1 .0));
        let parts: Vec<String> = kinds
            .iter()
            .map(|(kind, (count, turns))| {
                let mut turns = turns.clone();
                turns.sort_unstable();
                format!(
                    "{} {} ({:.0}%, median t{})",
                    if kind.is_empty() { "unfinished" } else { kind },
                    count,
                    100.0 * *count as f64 / screened_rows.len().max(1) as f64,
                    turns[turns.len() / 2]
                )
            })
            .collect();
        if !parts.is_empty() {
            println!("how the games ended: {}", parts.join(" · "));
        }
    }
    if !anchors.is_empty() {
        let (on, off): (Vec<&Row>, Vec<&Row>) = anchors.iter().partition(|row| row.arm == 0);
        let rate = |rows: &[&Row]| {
            if rows.is_empty() {
                0.0
            } else {
                rows.iter().filter(|row| row.win).count() as f64 / rows.len() as f64
            }
        };
        let share = |rows: &[&Row]| {
            if rows.is_empty() {
                0.0
            } else {
                rows.iter().map(|row| row.score_share).sum::<f64>() / rows.len() as f64
            }
        };
        println!(
            "anchors: all-on {} games win {:.1}% share {:.1}% · all-off {} games win {:.1}% share {:.1}%",
            on.len(),
            100.0 * rate(&on),
            100.0 * share(&on),
            off.len(),
            100.0 * rate(&off),
            100.0 * share(&off)
        );
    }
    if estimates.is_empty() {
        println!("no screened genes with complete pairs");
        return;
    }
    let k = estimates.len();
    let family_z = normal_quantile_upper(0.025 / k as f64);
    let median_se = {
        let mut ses: Vec<f64> = estimates
            .iter()
            .map(|e| e.win_se)
            .filter(|se| se.is_finite())
            .collect();
        ses.sort_by(|a, b| a.total_cmp(b));
        ses.get(ses.len() / 2).copied().unwrap_or(f64::INFINITY)
    };
    let median_share_se = {
        let mut ses: Vec<f64> = estimates
            .iter()
            .map(|e| e.share_se)
            .filter(|se| se.is_finite())
            .collect();
        ses.sort_by(|a, b| a.total_cmp(b));
        ses.get(ses.len() / 2).copied().unwrap_or(f64::INFINITY)
    };
    println!(
        "resolution: {} genes; this run resolves a win Δ of ±{:.1} pp (share Δ ±{:.2} pp) at 80% power; \
         |z|≥2 flags ~{:.1} genes by chance, family-wise 5% bar is |z|≥{:.2}",
        k,
        280.0 * median_se,
        280.0 * median_share_se,
        k as f64 * 0.0455,
        family_z
    );
    let adjusted_shown = estimates.iter().any(|e| e.adjusted.is_some());
    if !adjusted_shown {
        println!(
            "adjusted column needs at least {} pairs (2·genes+10) — showing marginal estimates only",
            2 * k + 10
        );
    }
    estimates.sort_by(|a, b| b.win_z().total_cmp(&a.win_z()));
    println!(
        "\n{:<28} {:>5} {:>6} {:>6} {:>7} {:>15} {:>6}  {:>8} {:>6}  {:>9}  read",
        "gene", "pairs", "on%", "off%", "Δpp", "95% CI", "z", "shareΔ", "z", "adjΔpp"
    );
    for e in &estimates {
        let z = e.win_z();
        let read = read_column(z, e.share_z(), family_z);
        let adjusted = match e.adjusted {
            Some((effect, se)) => format!("{:+.1}±{:.1}", 100.0 * effect, 100.0 * se),
            None => "-".to_string(),
        };
        println!(
            "{:<28} {:>5} {:>5.1}% {:>5.1}% {:>+7.1} [{:>+6.1},{:>+6.1}] {:>+6.2}  {:>+7.2}pp {:>+6.2}  {:>9}  {}",
            e.tag,
            e.pairs,
            100.0 * e.win_on,
            100.0 * e.win_off,
            100.0 * e.win_delta,
            100.0 * (e.win_delta - 1.96 * e.win_se),
            100.0 * (e.win_delta + 1.96 * e.win_se),
            z,
            100.0 * e.share_delta,
            e.share_z(),
            adjusted,
            read
        );
    }
    println!(
        "\n`*` = |z|≥2 (a screen flag, ~1 in 22 by chance); `**` = past the family-wise bar; the read \
         column names the win Δ first and the score-share Δ when it says more. `~` = unresolved at \
         this size, NOT no effect. shareΔ is the score-share Δ in points; adjΔpp is the OLS win Δ \
         over the whole sign matrix."
    );
}

fn read_rows(paths: &[String]) -> (Header, Vec<Row>) {
    let mut header: Option<Header> = None;
    let mut rows = Vec::new();
    for path in paths {
        let file = std::fs::File::open(path).unwrap_or_else(|error| {
            eprintln!("cannot open {path}: {error}");
            std::process::exit(2);
        });
        for (line_no, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line = line.unwrap_or_else(|error| {
                eprintln!("{path}:{}: {error}", line_no + 1);
                std::process::exit(2);
            });
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(found) = serde_json::from_str::<Header>(&line) {
                if found.kind == "header" {
                    match &header {
                        None => header = Some(found),
                        Some(first) => {
                            if first.genes != found.genes {
                                eprintln!(
                                    "{path} was written with a different gene order than {}; \
                                     it cannot be merged (regenerate both with the same build)",
                                    paths[0]
                                );
                                std::process::exit(2);
                            }
                            if first.players != found.players
                                || first.width != found.width
                                || first.height != found.height
                                || first.turns != found.turns
                                || first.speed != found.speed
                                || first.map != found.map
                                || first.baseline != found.baseline
                                || first.field != found.field
                                || first.randomize_civs != found.randomize_civs
                            {
                                eprintln!(
                                    "{path} was played at a different profile than {}; a merged \
                                     table would mix two experiments",
                                    paths[0]
                                );
                                std::process::exit(2);
                            }
                        }
                    }
                    continue;
                }
            }
            match serde_json::from_str::<Row>(&line) {
                Ok(row) => rows.push(row),
                Err(error) => {
                    eprintln!("{path}:{}: not a row: {error}", line_no + 1);
                    std::process::exit(2);
                }
            }
        }
    }
    let Some(header) = header else {
        eprintln!("no header line found; was this file written by gene_screen?");
        std::process::exit(2);
    };
    (header, rows)
}

fn usage() -> ! {
    eprintln!(
        "usage: gene_screen [--pairs N] [--start-seed N] [--players N] [--turns N] \
         [--width N] [--height N] [--city-states N] [--speed ID] [--map ID] [--jobs N] \
         [--genes tag,tag,...] [--baseline repairs|stock] [--field advanced|repairs] \
         [--anchor-pairs N] [--randomize-civs] [--out PATH] [--append] [--quiet]\n       \
         gene_screen --analyze PATH [PATH ...]\n       gene_screen --list"
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if present(&args, "--help") || present(&args, "-h") {
        usage();
    }
    let genes = gene_table();

    if present(&args, "--list") {
        println!("{} genes (bit order):", genes.len());
        for (i, gene) in genes.iter().enumerate() {
            println!(
                "{i:>3}  {:<28} {:<32} repairs:{} stock:{}",
                gene.tag,
                gene.field,
                if gene.after_setup_on { "on " } else { "off" },
                if gene.stock_on { "on" } else { "off" }
            );
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--analyze") {
        let paths: Vec<String> = args[index + 1..]
            .iter()
            .take_while(|arg| !arg.starts_with("--"))
            .cloned()
            .collect();
        if paths.is_empty() {
            eprintln!("--analyze needs at least one JSONL path");
            usage();
        }
        let (header, rows) = read_rows(&paths);
        print_table(&header, &rows);
        return;
    }

    let pairs = number(&args, "--pairs", 100).max(1) as usize;
    let anchor_pairs = number(&args, "--anchor-pairs", 0).max(0) as usize;
    let start_seed = number(&args, "--start-seed", 26_081_900) as u64;
    let players = number(&args, "--players", 4).max(2) as usize;
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let city_states = number(&args, "--city-states", 6).max(0) as usize;
    let jobs = number(&args, "--jobs", civvis::parallel::default_jobs() as i64).max(1) as usize;
    let quiet = present(&args, "--quiet");
    // ⚠ Stock seating is a FIXED civ per seat (Rome, Egypt, Greece, China…),
    // and on the first 250-pair run seats 0 and 2 won twice as often as seat 3
    // whoever sat there. The foldover cancels that for every per-gene contrast
    // — both arms share the seat — but the field is always the same three
    // civs unless this is on.
    let randomize_civs = present(&args, "--randomize-civs");
    let speed = match text(&args, "--speed") {
        None => GameSpeed::Online,
        Some(id) => GameSpeed::from_id(&id).unwrap_or_else(|| {
            eprintln!("unknown --speed {id:?}; use online|quick|standard|epic|marathon");
            std::process::exit(2);
        }),
    };
    let turns = if present(&args, "--turns") {
        number(&args, "--turns", 250).max(1) as u32
    } else {
        speed.turn_limit()
    };
    let map = match text(&args, "--map") {
        None => MapScript::Pangaea,
        Some(id) => MapScript::from_id(&id).unwrap_or_else(|| {
            eprintln!("unknown --map {id:?}");
            std::process::exit(2);
        }),
    };
    let baseline = match text(&args, "--baseline").as_deref() {
        None | Some("repairs") => Baseline::Repairs,
        Some("stock") => Baseline::Stock,
        Some(other) => {
            eprintln!("unknown --baseline {other:?}; use repairs|stock");
            std::process::exit(2);
        }
    };
    let field = match text(&args, "--field").as_deref() {
        None | Some("advanced") => Field::Advanced,
        Some("repairs") => Field::Repairs,
        Some(other) => {
            eprintln!("unknown --field {other:?}; use advanced|repairs");
            std::process::exit(2);
        }
    };
    let screened: Vec<bool> = match text(&args, "--genes") {
        None => vec![true; genes.len()],
        Some(list) => {
            let wanted: Vec<&str> = list
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            for name in &wanted {
                if !genes
                    .iter()
                    .any(|gene| gene.tag == *name || gene.field == *name)
                {
                    eprintln!("unknown gene {name:?}; `gene_screen --list` names them");
                    std::process::exit(2);
                }
            }
            genes
                .iter()
                .map(|gene| {
                    wanted
                        .iter()
                        .any(|name| gene.tag == *name || gene.field == *name)
                })
                .collect()
        }
    };
    let screened_count = screened.iter().filter(|&&s| s).count();
    if screened_count == 0 {
        eprintln!("nothing to screen");
        std::process::exit(2);
    }

    let out_path =
        text(&args, "--out").unwrap_or_else(|| format!("gene_screen-{start_seed}.jsonl"));
    let append = present(&args, "--append");
    if !append
        && std::fs::metadata(&out_path)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
    {
        eprintln!(
            "{out_path} already holds rows; pass --append to add to it (with a disjoint --start-seed) or --out for a new file"
        );
        std::process::exit(2);
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .unwrap_or_else(|error| {
            eprintln!("cannot open {out_path}: {error}");
            std::process::exit(2);
        });
    let header = Header {
        kind: "header".to_string(),
        genes: genes.iter().map(|gene| gene.tag.to_string()).collect(),
        screened: genes
            .iter()
            .zip(&screened)
            .filter(|(_, &s)| s)
            .map(|(gene, _)| gene.tag.to_string())
            .collect(),
        players,
        width,
        height,
        turns,
        city_states,
        speed: speed.id().to_string(),
        map: map.id().to_string(),
        baseline: format!("{baseline:?}").to_lowercase(),
        field: format!("{field:?}").to_lowercase(),
        start_seed,
        randomize_civs,
    };
    writeln!(
        out,
        "{}",
        serde_json::to_string(&header).expect("header serializes")
    )
    .expect("write header");

    let profile = Profile {
        players,
        width,
        height,
        turns,
        city_states,
        speed,
        map,
        randomize_civs,
    };
    println!(
        "gene screen: {pairs} foldover pairs ({} games){} · {} of {} genes screened · {players}p {width}x{height} {} · {} · {turns} turns · {city_states} city-states · {} civs · baseline {:?} · field {:?} · seeds {start_seed}..{} · {jobs} jobs · rows → {out_path}",
        pairs * 2,
        if anchor_pairs > 0 {
            format!(" + {anchor_pairs} anchor pairs")
        } else {
            String::new()
        },
        screened_count,
        genes.len(),
        map.id(),
        speed.id(),
        if randomize_civs { "shuffled" } else { "stock-seated" },
        baseline,
        field,
        start_seed + (pairs + anchor_pairs) as u64 - 1
    );

    // Job list: screened pairs first, then anchors, two games each. Every job
    // is independent, so the batch goes through the repository's pool.
    let total_games = 2 * (pairs + anchor_pairs);
    let all_on: Vec<bool> = genes
        .iter()
        .zip(&screened)
        .map(|(gene, &s)| if s { true } else { gene.after_setup_on })
        .collect();
    let all_off: Vec<bool> = genes
        .iter()
        .zip(&screened)
        .map(|(gene, &s)| if s { false } else { gene.after_setup_on })
        .collect();
    let started = Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let wins = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(out);
    let rows: Vec<Row> = civvis::parallel::map_reporting(
        total_games,
        jobs,
        |index| {
            let pair = index / 2;
            let arm = (index % 2) as u8;
            let seed = start_seed + pair as u64;
            let seat = pair % players;
            let (kind, genome) = if pair < pairs {
                let drawn = draw_genome(start_seed, pair, &screened);
                (
                    "game",
                    if arm == 0 {
                        drawn
                    } else {
                        complement(&drawn, &screened)
                    },
                )
            } else {
                (
                    "anchor",
                    if arm == 0 {
                        all_on.clone()
                    } else {
                        all_off.clone()
                    },
                )
            };
            play(
                &profile, &genes, &screened, baseline, field, kind, pair, arm, seed, seat, &genome,
            )
        },
        |_index, row| {
            let mut out = out.lock().expect("row writer");
            writeln!(
                out,
                "{}",
                serde_json::to_string(row).expect("row serializes")
            )
            .expect("write row");
            out.flush().expect("flush rows");
            let finished = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if row.win {
                wins.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if !quiet {
                let elapsed = started.elapsed().as_secs_f64();
                println!(
                    "[{finished:>5}/{total_games}] {} pair {} arm {} seed {} seat {} · {} · t{} · win={} share={:.1}% rank {} cities {} · {:.0}s ({:.2} games/s, ~{:.0}s left)",
                    row.kind,
                    row.pair,
                    row.arm,
                    row.seed,
                    row.seat,
                    if row.victory.is_empty() { "-" } else { &row.victory },
                    row.turn,
                    u8::from(row.win),
                    100.0 * row.score_share,
                    row.rank,
                    row.cities,
                    row.secs,
                    finished as f64 / elapsed.max(1e-9),
                    elapsed / finished as f64 * (total_games - finished) as f64
                );
            }
        },
    );
    println!(
        "\n{} games in {:.0}s ({:.2} games/s); treated seat won {} of them",
        rows.len(),
        started.elapsed().as_secs_f64(),
        rows.len() as f64 / started.elapsed().as_secs_f64().max(1e-9),
        wins.load(std::sync::atomic::Ordering::Relaxed)
    );
    print_table(&header, &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gene table is discovered from the repository's tables and every
    /// row can actually be flipped on a real controller.
    #[test]
    fn every_gene_is_a_real_flag_with_a_toggle() {
        let genes = gene_table();
        assert_eq!(
            genes.len(),
            civvis::elo::ENGINE_REPAIR_TREATMENTS.len()
                + civvis::ai::PRODUCTION_TREATMENTS.len()
                + civvis::ai::PRODUCTION_OPT_INS.len()
        );
        let mut tags: Vec<&str> = genes.iter().map(|g| g.tag).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), genes.len(), "a gene tag is repeated");
        // Firaxis-only flags are excluded by construction, not by luck.
        for gene in &genes {
            assert!(
                !civvis::elo::FIRAXIS_ONLY_TREATMENTS.contains(&gene.tag),
                "{} is host-only and would screen as noise",
                gene.tag
            );
        }
        // Flipping every gene on a live controller must not panic.
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        for gene in &genes {
            (gene.flip)(&mut ai);
        }
    }

    #[test]
    fn a_pair_is_a_foldover_and_reproduces_from_its_seed() {
        let screened = vec![true, true, false, true];
        let a = draw_genome(7, 3, &screened);
        let b = draw_genome(7, 3, &screened);
        assert_eq!(a, b, "the same seed and pair must draw the same genome");
        assert!(!a[2], "an un-screened gene is never drawn on");
        let c = complement(&a, &screened);
        for i in [0, 1, 3] {
            assert_ne!(a[i], c[i], "screened gene {i} must flip");
        }
        assert_eq!(a[2], c[2], "un-screened gene must not flip");
        assert_eq!(complement(&c, &screened), a);
        assert_eq!(genome_string(&[true, false, true]), "101");
    }

    /// Over many pairs a screened gene is on about half the time — the
    /// property the whole per-gene comparison rests on.
    #[test]
    fn genes_are_balanced_across_pairs() {
        let screened = vec![true; 8];
        let mut on = vec![0usize; 8];
        let pairs = 2000;
        for pair in 0..pairs {
            for (i, &bit) in draw_genome(99, pair, &screened).iter().enumerate() {
                on[i] += usize::from(bit);
            }
        }
        for (i, count) in on.iter().enumerate() {
            let rate = *count as f64 / pairs as f64;
            assert!(
                (0.45..=0.55).contains(&rate),
                "gene {i} on-rate {rate} is not near one half"
            );
        }
    }

    #[test]
    fn treated_seat_respects_the_baseline_for_unscreened_genes() {
        // Nothing observable is exposed for most flags, so this test pins the
        // logic on the one flag that is public: `siege_is_progress` is an
        // engine repair, on after setup, off on stock.
        let genes = gene_table();
        let index = genes
            .iter()
            .position(|g| g.tag == "siege-is-progress")
            .expect("siege-is-progress is an engine repair");
        let none_screened = vec![false; genes.len()];
        let genome = vec![false; genes.len()];
        let repairs = treated_seat(&genes, &genome, &none_screened, Baseline::Repairs);
        assert!(
            repairs.siege_is_progress,
            "repairs baseline keeps the repair on"
        );
        let stock = treated_seat(&genes, &genome, &none_screened, Baseline::Stock);
        assert!(
            !stock.siege_is_progress,
            "stock baseline turns the repair off"
        );
        // Screened: the genome bit wins over either baseline.
        let mut one_screened = vec![false; genes.len()];
        one_screened[index] = true;
        let mut on = vec![false; genes.len()];
        on[index] = true;
        assert!(treated_seat(&genes, &on, &one_screened, Baseline::Stock).siege_is_progress);
        let off = vec![false; genes.len()];
        assert!(!treated_seat(&genes, &off, &one_screened, Baseline::Repairs).siege_is_progress);
    }

    #[test]
    fn the_read_column_names_both_axes() {
        assert_eq!(read_column(0.5, -0.3, 3.33), "~");
        assert_eq!(read_column(2.4, 0.1, 3.33), "helps *");
        assert_eq!(read_column(-0.8, -7.2, 3.33), "share HURTS **");
        assert_eq!(read_column(2.1, 4.9, 3.33), "helps * · share HELPS **");
        assert_eq!(read_column(-3.5, -1.0, 3.33), "HURTS **");
    }

    #[test]
    fn the_normal_tables_are_right() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((normal_quantile_upper(0.025) - 1.96).abs() < 1e-3);
        assert!((normal_quantile_upper(0.025 / 57.0) - 3.33).abs() < 0.02);
    }

    #[test]
    fn ols_recovers_planted_effects() {
        // 400 pairs, 5 genes, planted effects; noise-free so the solve is exact.
        let planted = [0.3, -0.2, 0.0, 0.1, 0.05];
        let screened = vec![true; 5];
        let mut signs = Vec::new();
        let mut diffs = Vec::new();
        for pair in 0..400 {
            let g = draw_genome(5, pair, &screened);
            let row: Vec<f64> = g.iter().map(|&b| if b { 1.0 } else { -1.0 }).collect();
            let d: f64 = row.iter().zip(&planted).map(|(s, e)| s * e).sum();
            signs.push(row);
            diffs.push(d);
        }
        let fitted = adjusted_effects(&signs, &diffs).expect("400 pairs support 5 genes");
        for (i, (effect, se)) in fitted.iter().enumerate() {
            assert!(
                (effect - planted[i]).abs() < 1e-9,
                "gene {i}: {effect} vs {}",
                planted[i]
            );
            assert!(*se < 1e-6);
        }
        assert!(
            adjusted_effects(&signs[..15], &diffs[..15]).is_none(),
            "too few pairs for the design"
        );
    }

    /// The estimator reads a planted gene out of synthetic rows: on-arm wins
    /// more often, and the pair difference carries the sign.
    #[test]
    fn estimate_reads_a_planted_gene() {
        let genes = vec!["a".to_string(), "b".to_string()];
        let header = Header {
            kind: "header".into(),
            genes: genes.clone(),
            screened: genes,
            players: 4,
            width: 1,
            height: 1,
            turns: 1,
            city_states: 0,
            speed: "online".into(),
            map: "pangaea".into(),
            baseline: "repairs".into(),
            field: "advanced".into(),
            start_seed: 1,
            randomize_civs: false,
        };
        let mut rows = Vec::new();
        let screened = vec![true, true];
        for pair in 0..300 {
            let g = draw_genome(11, pair, &screened);
            let c = complement(&g, &screened);
            // Gene `a` on wins the pair; gene `b` does nothing.
            for (arm, genome) in [(0u8, &g), (1u8, &c)] {
                rows.push(Row {
                    kind: "game".into(),
                    pair,
                    arm,
                    seed: pair as u64,
                    seat: pair % 4,
                    genome: genome_string(genome),
                    win: genome[0],
                    winner: None,
                    victory: String::new(),
                    turn: 1,
                    score: 0,
                    score_share: if genome[0] { 0.4 } else { 0.2 },
                    rank: 1,
                    cities: 0,
                    alive: true,
                    secs: 0.0,
                });
            }
        }
        let (estimates, pairs, overall_win, _) = estimate(&header, &rows);
        assert_eq!(pairs, 300);
        assert!((overall_win - 0.5).abs() < 1e-9);
        let a = estimates.iter().find(|e| e.tag == "a").unwrap();
        let b = estimates.iter().find(|e| e.tag == "b").unwrap();
        assert!((a.win_on - 1.0).abs() < 1e-9 && a.win_off.abs() < 1e-9);
        assert!((a.win_delta - 1.0).abs() < 1e-9);
        assert!((a.share_delta - 0.2).abs() < 1e-9);
        assert!(
            b.win_delta.abs() < 0.15,
            "b is planted null: {}",
            b.win_delta
        );
        assert!(b.win_z().abs() < 2.5);
        let (adj_a, _) = a.adjusted.expect("300 pairs support 2 genes");
        assert!((adj_a - 1.0).abs() < 1e-9);
        // An unfinished pair (one arm only) is dropped, not counted.
        rows.push(Row {
            pair: 999,
            arm: 0,
            seed: 999,
            ..rows[0].clone()
        });
        assert_eq!(estimate(&header, &rows).1, 300);
    }

    #[test]
    fn rows_round_trip_through_json() {
        let row = Row {
            kind: "game".into(),
            pair: 3,
            arm: 1,
            seed: 42,
            seat: 2,
            genome: "0110".into(),
            win: true,
            winner: Some(2),
            victory: "science".into(),
            turn: 210,
            score: 1234,
            score_share: 0.31,
            rank: 1,
            cities: 9,
            alive: true,
            secs: 12.5,
        };
        let text = serde_json::to_string(&row).unwrap();
        let back: Row = serde_json::from_str(&text).unwrap();
        assert_eq!(back.genome, "0110");
        assert_eq!(back.winner, Some(2));
        assert!(serde_json::from_str::<Header>(&text)
            .map(|h| h.kind != "header")
            .unwrap_or(true));
    }
}
