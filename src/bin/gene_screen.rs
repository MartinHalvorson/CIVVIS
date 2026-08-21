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
//! - three newest-first, non-overlapping ≈10,000-pair win tranches. They make
//!   an apparent win or loss auditable for replication before it changes the
//!   genome; every seat from one all-seats map pair stays in the same tranche.
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
//!   gene_screen --analyze PATH [PATH ...] [--interactions] [--top N]
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
    /// On after `enable_engine_repairs_universe` — the genome's universe.
    after_setup_on: bool,
    stock_on: bool,
    /// On in the deployment genome: the ledger's `helps`, or — for a gene the
    /// ledger has not measured — the universe state. See `gene_ledger.rs`.
    default_on: bool,
    flip: fn(&mut AdvancedAi),
}

/// The deployment default for a tag: the ledger's say, else the universe.
fn ledger_default(tag: &str, universe_on: bool) -> bool {
    civvis::ai::ledger_default_on(tag).unwrap_or(universe_on)
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
            default_on: ledger_default(tag, true),
            flip: disable,
        });
    }
    for &(field, tag, disable) in civvis::ai::PRODUCTION_TREATMENTS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: true,
            stock_on: true,
            default_on: ledger_default(tag, true),
            flip: disable,
        });
    }
    for &(field, tag, enable) in civvis::ai::PRODUCTION_OPT_INS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: false,
            stock_on: false,
            default_on: ledger_default(tag, false),
            flip: enable,
        });
    }
    genes
}

/// How the on-probability of each screened gene is chosen.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Design {
    /// Each gene on with probability one half; arm 1 is arm 0's exact
    /// complement. Every gene on in exactly one arm of every pair.
    Foldover,
    /// ★★★★ THE HELPFUL GENES PLAY MOST OF THE TIME, AND ARE STILL PRICED.
    /// Operator directive 2026-08-20: in the large batch tests a helpful gene
    /// may be on in 90% of games and a harmful one in 10%, and the win rate
    /// of the 90% is still compared with the win rate of the 10% — for a
    /// helper the 90% should win more. Each arm is drawn independently from
    /// the prior (both arms still share the map and seat), the per-gene
    /// contrast is the marginal on-versus-off difference with errors
    /// clustered by game, and the adjusted column is the map-paired OLS on
    /// the arms' differences, which cancels the map exactly where the arms
    /// differ on a gene.
    Prior,
}

impl Design {
    fn id(self) -> &'static str {
        match self {
            Design::Foldover => "foldover",
            Design::Prior => "prior",
        }
    }
}

/// The on-probability of a gene under the prior design, from its ledger
/// verdict: helps 0.9, hurts 0.1, unresolved (or unmeasured) 0.5 unless the
/// operator moves them.
#[derive(Clone, Copy, Debug)]
struct PriorWeights {
    helps: f64,
    hurts: f64,
    unresolved: f64,
}

impl PriorWeights {
    fn for_tag(&self, tag: &str) -> f64 {
        match civvis::ai::ledger_verdict(tag).map(|row| row.verdict) {
            Some(civvis::ai::Verdict::Helps) => self.helps,
            Some(civvis::ai::Verdict::Hurts) => self.hurts,
            _ => self.unresolved,
        }
    }
}

/// What the un-screened genes are held at.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Baseline {
    /// The deployment genome: every gene at the ledger's default (helps on,
    /// the rest off; unmeasured genes as the universe set them). The default.
    Best,
    /// The genome's universe: every engine repair on.
    Repairs,
    /// Production `advanced`: every engine repair off.
    Stock,
}

/// Who the treated seat plays against.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Field {
    /// Production `advanced`, the ladder's incumbent.
    Advanced,
    /// The native repair bundle as deployed (`advanced_synergy`, ledger applied).
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
    /// ★ WHY THE SEAT LOST, NOT ONLY THAT IT DID. The first 300-pair run's
    /// census said 66% of native games end by RELIGIOUS conversion at a median
    /// of turn 149 — the single largest failure mode on the board — and the
    /// rows could not say one thing about how the losing seat stood in that
    /// race. These fields are the cheapest possible answer, all end-of-game
    /// reads, and they turn "lost to religion" into a diagnosis: did it found
    /// a faith at all, how many of its own cities were flying a foreign one at
    /// the end, was the faith banked rather than spent, and had it ever
    /// unlocked the Inquisitor.
    ///
    /// `#[serde(default)]` on every one of them, so a file written before they
    /// existed still analyses.
    #[serde(default)]
    founded_religion: bool,
    /// Our cities whose MAJORITY religion is somebody else's faith.
    #[serde(default)]
    foreign_faith_cities: usize,
    /// Faith still in the bank at the end. A seat losing the conversion race
    /// with a thousand faith unspent is a spending defect, not a race it could
    /// not have run.
    #[serde(default)]
    faith: f64,
    /// Whether an Inquisition was ever launched — the gate that has to fall
    /// before an Inquisitor can be bought at all.
    #[serde(default)]
    inquisition: bool,
    #[serde(default)]
    techs: usize,
    #[serde(default)]
    military: f64,
    /// The civilization this seat played. Both arms of a pair share it — the
    /// roster shuffle is seeded by the map seed — so per-civ contrasts stay
    /// paired. Empty in files written before the field existed.
    #[serde(default)]
    civ: String,
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
    /// The victory lanes left enabled, comma separated. Absent in files written
    /// before the flag existed, which means all six.
    #[serde(default)]
    victories: String,
    /// Every major seat carries its own drawn genome (arm 1 complements all of
    /// them), so each game yields one observation per major instead of one per
    /// game. Absent in files written before the mode existed, which means the
    /// classic one-treated-seat design.
    #[serde(default)]
    all_seats: bool,
    /// `foldover` or `prior`. Absent in files written before the prior
    /// design existed, which means foldover.
    #[serde(default = "foldover")]
    design: String,
    /// Under the prior design, each gene's on-probability in header order
    /// (un-screened genes carry 0 or 1 for their held state). Empty for a
    /// foldover file.
    #[serde(default)]
    prior: Vec<f64>,
}

fn foldover() -> String {
    "foldover".to_string()
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

/// One arm's genome under the prior design: each screened gene on with its
/// own probability, seeded from the start seed, the pair and the ARM, so the
/// two arms of a pair are independent draws on the same map and seat.
fn draw_genome_prior(
    start_seed: u64,
    pair: usize,
    arm: u8,
    screened: &[bool],
    prior: &[f64],
) -> Vec<bool> {
    let mut rng = Rng::new(
        start_seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((pair as u64) * 2 + u64::from(arm))
            .wrapping_add(0x9E4E_5EED),
    );
    screened
        .iter()
        .zip(prior)
        .map(|(&is_screened, &p)| is_screened && rng.chance(p))
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
    // The universe, not the deployment genome: every gene is set explicitly
    // below, and `after_setup_on` describes the universe.
    ai.enable_engine_repairs_universe();
    for ((gene, &on), &is_screened) in genes.iter().zip(genome).zip(screened) {
        let desired = if is_screened {
            on
        } else {
            match baseline {
                Baseline::Best => gene.default_on,
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
    victories: civvis::game::VictoryConditions,
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
        victory_conditions: profile.victories,
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

    row_for_seat(
        &game,
        kind,
        pair,
        arm,
        seed,
        seat,
        genome_string(genome),
        started.elapsed().as_secs_f64(),
    )
}

/// One finished game read from one seat's point of view.
#[allow(clippy::too_many_arguments)]
fn row_for_seat(
    game: &Game,
    kind: &str,
    pair: usize,
    arm: u8,
    seed: u64,
    seat: usize,
    genome: String,
    secs: f64,
) -> Row {
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
    let religion = game.players[seat].religion.clone();
    let foreign_faith_cities = game
        .player_city_ids(seat)
        .iter()
        .filter_map(|cid| game.cities.get(cid))
        .filter(|city| {
            game.city_religion(city)
                .is_some_and(|faith| Some(faith) != religion.as_deref())
        })
        .count();
    Row {
        kind: kind.to_string(),
        pair,
        arm,
        seed,
        seat,
        genome,
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
        secs,
        founded_religion: religion.is_some(),
        foreign_faith_cities,
        faith: game.players[seat].faith,
        inquisition: game.players[seat]
            .counters
            .get("inquisition")
            .is_some_and(|launched| *launched > 0),
        techs: game.players[seat].techs.len(),
        military: game.military_power(seat),
        civ: game.players[seat].civ.clone(),
    }
}

/// The per-seat genomes of one all-seats game: seat `s` draws from the
/// screen's seed stream at index `pair * players + s`, and arm 1 complements
/// every seat. Deterministic in `(start_seed, pair, seat)`, so a run
/// reproduces exactly and `--append` on a disjoint seed window draws disjoint
/// genomes, exactly as in the classic design.
fn all_seat_genomes(
    start_seed: u64,
    pair: usize,
    players: usize,
    screened: &[bool],
    arm: u8,
    design: Design,
    prior: &[f64],
) -> Vec<Vec<bool>> {
    (0..players)
        .map(|seat| match design {
            Design::Foldover => {
                let drawn = draw_genome(start_seed, pair * players + seat, screened);
                if arm == 0 {
                    drawn
                } else {
                    complement(&drawn, screened)
                }
            }
            Design::Prior => {
                draw_genome_prior(start_seed, pair * players + seat, arm, screened, prior)
            }
        })
        .collect()
}

/// Play one game in which EVERY major seat carries its own drawn genome, and
/// report one row per major. Minor and barbarian seats stay stock. The field
/// is the other treated majors — effects are averaged over random opposing
/// genomes rather than measured against a fixed production field, which is
/// the point: a flag that only pays against untreated opponents is a flag
/// the mixed ecosystem does not have.
#[allow(clippy::too_many_arguments)]
fn play_all_seats(
    profile: &Profile,
    genes: &[Gene],
    screened: &[bool],
    baseline: Baseline,
    pair: usize,
    arm: u8,
    seed: u64,
    genomes: &[Vec<bool>],
) -> Vec<Row> {
    let started = Instant::now();
    let mut game = Game::new_with(GameOptions {
        speed: profile.speed.id().to_string(),
        map_script: profile.map,
        randomize_civs: profile.randomize_civs,
        victory_conditions: profile.victories,
        ..GameOptions::new(
            profile.players,
            profile.width,
            profile.height,
            seed,
            profile.turns,
            profile.city_states,
        )
    });
    let mut majors = Vec::new();
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|pid| {
            if game.players[pid].is_minor || game.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                majors.push(pid);
                treated_seat(genes, &genomes[majors.len() - 1], screened, baseline)
            }
        })
        .collect();
    run_game(&mut game, &mut ais);
    let secs = started.elapsed().as_secs_f64();
    majors
        .iter()
        .enumerate()
        .map(|(index, &seat)| {
            row_for_seat(
                &game,
                "game",
                pair,
                arm,
                seed,
                seat,
                genome_string(&genomes[index]),
                secs,
            )
        })
        .collect()
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

/// Mean and standard error of paired differences, clustered: observations
/// sharing a `(seed, pair)` key are averaged into one number first, so seat
/// pairs from the same all-seats game — whose outcomes share one winner —
/// contribute one observation, not `players` correlated ones. With singleton
/// clusters (the classic design) this IS `mean_se`, value for value.
fn clustered_mean_se(values: &[f64], clusters: &[(u64, usize)]) -> (f64, f64) {
    let mut grouped: BTreeMap<(u64, usize), (f64, usize)> = BTreeMap::new();
    for (value, key) in values.iter().zip(clusters) {
        let slot = grouped.entry(*key).or_insert((0.0, 0));
        slot.0 += value;
        slot.1 += 1;
    }
    let means: Vec<f64> = grouped.values().map(|(sum, n)| sum / *n as f64).collect();
    mean_se(&means)
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
fn adjusted_effects(
    signs: &[Vec<f64>],
    diffs: &[f64],
    clusters: &[(u64, usize)],
) -> Option<Vec<(f64, f64)>> {
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
    let residuals: Vec<f64> = signs
        .iter()
        .zip(diffs)
        .map(|(row, &d)| {
            let fitted: f64 = row.iter().zip(&beta).map(|(s, b)| s * b).sum();
            d - fitted
        })
        .collect();
    let mut cluster_ids: Vec<(u64, usize)> = clusters.to_vec();
    cluster_ids.sort_unstable();
    cluster_ids.dedup();
    if cluster_ids.len() == n {
        // Singleton clusters: the classic design. Keep the homoskedastic OLS
        // standard error the column has always printed, value for value.
        let rss: f64 = residuals.iter().map(|e| e * e).sum();
        let sigma2 = rss / (n - k) as f64;
        return Some(
            (0..k)
                .map(|i| (beta[i], (sigma2 * inverse[i][i]).max(0.0).sqrt()))
                .collect(),
        );
    }
    // All-seats files: seat pairs from one game share a winner, so their OLS
    // residuals are correlated and the homoskedastic error would be too
    // small. The cluster-robust (sandwich) variance sums each game pair's
    // score vector `Xᵍᵀeᵍ` whole before squaring:
    // `(XᵀX)⁻¹ (Σᵍ sᵍ sᵍᵀ) (XᵀX)⁻¹`.
    let mut scores: BTreeMap<(u64, usize), Vec<f64>> = BTreeMap::new();
    for ((row, &e), key) in signs.iter().zip(&residuals).zip(clusters) {
        let score = scores.entry(*key).or_insert_with(|| vec![0.0; k]);
        for i in 0..k {
            score[i] += row[i] * e;
        }
    }
    let mut meat = vec![vec![0.0; k]; k];
    for score in scores.values() {
        for i in 0..k {
            for j in 0..k {
                meat[i][j] += score[i] * score[j];
            }
        }
    }
    Some(
        (0..k)
            .map(|i| {
                let variance: f64 = (0..k)
                    .map(|a| {
                        (0..k)
                            .map(|b| inverse[i][a] * meat[a][b] * inverse[b][i])
                            .sum::<f64>()
                    })
                    .sum();
                (beta[i], variance.max(0.0).sqrt())
            })
            .collect(),
    )
}

/// One gene's estimates from the pairs.
#[derive(Clone, Debug)]
struct GeneEstimate {
    tag: String,
    pairs: usize,
    /// Games with the gene on / off behind `win_on` / `win_off`. Equal to
    /// `pairs` each under the foldover; unequal under a prior-weighted draw.
    n_on: usize,
    n_off: usize,
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

/// A chronological replication tranche contains this many complete on/off
/// comparisons, rounded only enough to keep an all-seats game pair whole.
/// The latter is important: seats from one game share a winner, so putting
/// them into two tranches would make the purported replications correlated.
const REPRO_WINDOW_PAIRS: usize = 10_000;
const REPRO_WINDOW_COUNT: usize = 3;

/// A complete foldover pair plus its position in the JSONL stream. `ordinal`
/// is deliberately the input order rather than the seed: appended runs can
/// use any disjoint seed range, while input order is what "latest 10k" means.
#[derive(Clone, Copy)]
struct CompletePair<'a> {
    a: &'a Row,
    b: &'a Row,
    ordinal: usize,
}

/// One newest-first, non-overlapping replication tranche.
#[derive(Clone, Debug)]
struct ReproTranche {
    pairs: usize,
    estimates: Vec<GeneEstimate>,
}

/// The screened rows grouped into complete foldover pairs, in (arm 0, arm 1)
/// order and annotated with their position in the input stream.
///
/// A pair is complete when both arms are present for one `(seed, seat, pair)`
/// key; an unfinished run's odd row is dropped rather than counted as an
/// unpaired game. Merged files may repeat a key only if they replayed the same
/// pair, in which case the later row wins.
fn complete_pairs_with_order(rows: &[Row]) -> Vec<CompletePair<'_>> {
    let mut pairs: BTreeMap<(u64, usize, usize), [Option<(usize, &Row)>; 2]> = BTreeMap::new();
    for (ordinal, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.kind == "game")
    {
        let slot = pairs
            .entry((row.seed, row.seat, row.pair))
            .or_insert([None, None]);
        slot[usize::from(row.arm.min(1))] = Some((ordinal, row));
    }
    pairs
        .values()
        .filter_map(|[a, b]| {
            let (a_ordinal, a) = (*a)?;
            let (b_ordinal, b) = (*b)?;
            Some(CompletePair {
                a,
                b,
                ordinal: a_ordinal.max(b_ordinal),
            })
        })
        .collect()
}

fn complete_pairs(rows: &[Row]) -> Vec<(&Row, &Row)> {
    complete_pairs_with_order(rows)
        .into_iter()
        .map(|pair| (pair.a, pair.b))
        .collect()
}

/// Split the newest data into non-overlapping, chronological replication
/// tranches. A whole `(seed, pair)` cluster moves as one unit; in all-seats
/// files that is every seat from the two games on one map. The requested
/// width is therefore approximate only when it would otherwise split a
/// correlated cluster.
fn reproducibility_tranches(
    header: &Header,
    rows: &[Row],
    window_pairs: usize,
    window_count: usize,
) -> Vec<ReproTranche> {
    let mut grouped: BTreeMap<(u64, usize), Vec<CompletePair<'_>>> = BTreeMap::new();
    for pair in complete_pairs_with_order(rows) {
        grouped
            .entry((pair.a.seed, pair.a.pair))
            .or_default()
            .push(pair);
    }
    let mut clusters: Vec<(usize, Vec<CompletePair<'_>>)> = grouped
        .into_values()
        .map(|pairs| {
            let ordinal = pairs.iter().map(|pair| pair.ordinal).max().unwrap_or(0);
            (ordinal, pairs)
        })
        .collect();
    // `complete_pairs_with_order` preserves later-row-wins semantics; the
    // ordinal then makes this correct for both --append and several --analyze
    // input files, even if their disjoint seed ranges are not increasing.
    clusters.sort_by_key(|(ordinal, _)| *ordinal);

    let target = window_pairs.max(1);
    let mut end = clusters.len();
    let mut tranches = Vec::new();
    while end > 0 && tranches.len() < window_count {
        let mut start = end;
        let mut count = 0usize;
        while start > 0 && count < target {
            start -= 1;
            count += clusters[start].1.len();
        }
        // Choose the closest boundary, but never create an empty window. For
        // six treated seats a 10,000-pair target becomes 10,002 rather than
        // splitting four seats from the next map pair away from their peers.
        if count > target {
            let without_first = count - clusters[start].1.len();
            if without_first > 0 && target - without_first < count - target {
                start += 1;
            }
        }
        let selected: Vec<CompletePair<'_>> = clusters[start..end]
            .iter()
            .flat_map(|(_, pairs)| pairs.iter().copied())
            .collect();
        let mut tranche_rows = Vec::with_capacity(selected.len() * 2);
        for pair in selected {
            tranche_rows.push(pair.a.clone());
            tranche_rows.push(pair.b.clone());
        }
        let (estimates, pairs, _, _) = estimate(header, &tranche_rows);
        tranches.push(ReproTranche { pairs, estimates });
        end = start;
    }
    tranches
}

fn tranche_estimate<'a>(tranche: &'a ReproTranche, tag: &str) -> Option<&'a GeneEstimate> {
    tranche
        .estimates
        .iter()
        .find(|estimate| estimate.tag == tag)
}

/// A compact table cell for one chronological win-rate replication tranche.
/// The effect is in percentage points and the z retains the window's paired,
/// cluster-aware uncertainty rather than pretending the raw seat rows are
/// independent.
fn tranche_cell(tranche: Option<&ReproTranche>, tag: &str) -> String {
    match tranche.and_then(|tranche| tranche_estimate(tranche, tag)) {
        Some(estimate) => format!(
            "{:+.1}pp z{:+.2}",
            100.0 * estimate.win_delta,
            estimate.win_z()
        ),
        None => "—".to_string(),
    }
}

/// The ±1 sign vector of a pair's arm-0 genome, restricted to screened genes,
/// or `None` when the genome string does not match the header's gene order.
fn pair_signs(genome: &str, screened: &[bool], k: usize) -> Option<Vec<f64>> {
    let bits: Vec<bool> = genome.chars().map(|c| c == '1').collect();
    if bits.len() != k {
        return None;
    }
    Some(
        (0..k)
            .filter(|&i| screened[i])
            .map(|i| if bits[i] { 1.0 } else { -1.0 })
            .collect(),
    )
}

/// Which header genes were screened, as a mask over the gene order.
fn screened_mask(header: &Header) -> Vec<bool> {
    header
        .genes
        .iter()
        .map(|gene| header.screened.contains(gene))
        .collect()
}

fn estimate(header: &Header, rows: &[Row]) -> (Vec<GeneEstimate>, usize, f64, f64) {
    if header.design == "prior" {
        return estimate_prior(header, rows);
    }
    let k = header.genes.len();
    let complete = complete_pairs(rows);
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

    let screened = screened_mask(header);
    let mut signs: Vec<Vec<f64>> = Vec::with_capacity(complete.len());
    let mut win_diffs: Vec<f64> = Vec::with_capacity(complete.len());
    let mut share_diffs: Vec<f64> = Vec::with_capacity(complete.len());
    // The game pair each seat pair came from. In the classic design every
    // cluster is one seat pair and the clustering below is the identity; in
    // an all-seats file one game holds `players` seat pairs whose outcomes
    // share a single winner, and treating them as independent would
    // understate every standard error.
    let mut clusters: Vec<(u64, usize)> = Vec::with_capacity(complete.len());
    for (a, b) in &complete {
        let Some(row_signs) = pair_signs(&a.genome, &screened, k) else {
            continue;
        };
        if b.genome.chars().count() != k {
            continue;
        }
        signs.push(row_signs);
        win_diffs.push(f64::from(u8::from(a.win)) - f64::from(u8::from(b.win)));
        share_diffs.push(a.score_share - b.score_share);
        clusters.push((a.seed, a.pair));
    }
    let adjusted = adjusted_effects(&signs, &win_diffs, &clusters);

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
        let (win_delta, win_se) = clustered_mean_se(&oriented_win, &clusters);
        let (share_delta, share_se) = clustered_mean_se(&oriented_share, &clusters);
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
            n_on: n,
            n_off: n,
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

/// The analysis as data: one object per screened gene with the numbers the
/// table prints, plus the profile. `tools/gene_ledger.py` reads this to
/// build `docs/gene_ledger.json` and the generated Rust table, so the
/// deployment genome is derived from the screens rather than typed in.
fn write_json_summary(path: &str, header: &Header, rows: &[Row]) {
    let (estimates, pairs, overall_win, overall_share) = estimate(header, rows);
    let tranches = reproducibility_tranches(header, rows, REPRO_WINDOW_PAIRS, REPRO_WINDOW_COUNT);
    let family_z = family_wise_z(estimates.len());
    let genes: Vec<serde_json::Value> = estimates
        .iter()
        .map(|e| {
            let win_tranches: Vec<serde_json::Value> = tranches
                .iter()
                .enumerate()
                .filter_map(|(index, tranche)| {
                    let estimate = tranche_estimate(tranche, &e.tag)?;
                    Some(serde_json::json!({
                        "position": match index {
                            0 => "latest",
                            1 => "previous",
                            _ => "earlier",
                        },
                        "pairs": tranche.pairs,
                        "win_delta_pp": 100.0 * estimate.win_delta,
                        "win_se_pp": 100.0 * estimate.win_se,
                        "win_z": estimate.win_z(),
                    }))
                })
                .collect();
            serde_json::json!({
                "tag": e.tag,
                "pairs": e.pairs,
                "n_on": e.n_on,
                "n_off": e.n_off,
                "win_on": e.win_on,
                "win_off": e.win_off,
                "win_delta_pp": 100.0 * e.win_delta,
                "win_se_pp": 100.0 * e.win_se,
                "win_z": e.win_z(),
                "share_delta_pp": 100.0 * e.share_delta,
                "share_se_pp": 100.0 * e.share_se,
                "share_z": e.share_z(),
                "adjusted_pp": e.adjusted.map(|(b, _)| 100.0 * b),
                "adjusted_se_pp": e.adjusted.map(|(_, se)| 100.0 * se),
                "read": read_column(e.win_z(), e.share_z(), family_z),
                "win_tranches": win_tranches,
            })
        })
        .collect();
    let summary = serde_json::json!({
        "kind": "gene_screen_analysis",
        "profile": header,
        "regime": if header.victories.is_empty()
            || header.victories.split(',').count() == civvis::game::VictoryConditions::NAMES.len()
        {
            "native".to_string()
        } else {
            header.victories.clone()
        },
        "complete_pairs": pairs,
        "overall_win": overall_win,
        "overall_share": overall_share,
        "family_wise_z": family_z,
        "reproducibility": {
            "unit": "complete paired comparisons",
            "target_pairs_per_window": REPRO_WINDOW_PAIRS,
            "windows": "newest first; whole game-pair clusters only",
        },
        "genes": genes,
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&summary).expect("summary serializes"),
    )
    .unwrap_or_else(|error| {
        eprintln!("cannot write {path}: {error}");
        std::process::exit(2);
    });
    println!("analysis written to {path}");
}

/// The family-wise 5% bar for `k` genes (Bonferroni, two-sided).
fn family_wise_z(k: usize) -> f64 {
    normal_quantile_upper(0.025 / k.max(1) as f64)
}

/// The prior design's estimates: the marginal on-versus-off contrast over
/// every game (the 90% against the 10%, errors clustered by game), and the
/// map-paired OLS on the arms' differences as the adjusted column.
///
/// Unlike the foldover, a pair's two arms are independent draws, so per-gene
/// balance is no longer exact and the map's own difficulty does not cancel
/// in the marginal contrast; it does cancel in the adjusted one, which
/// regresses `y₀ − y₁` on `x₀ − x₁ ∈ {−1, 0, +1}` — zero for every gene the
/// two arms agree on, so each gene is priced from the pairs that differ on
/// it, with the rest of the genome differenced out.
fn estimate_prior(header: &Header, rows: &[Row]) -> (Vec<GeneEstimate>, usize, f64, f64) {
    let k = header.genes.len();
    let screened = screened_mask(header);
    let games: Vec<&Row> = rows
        .iter()
        .filter(|row| row.kind == "game" && row.genome.chars().count() == k)
        .collect();
    let n = games.len();
    let overall_win = if n > 0 {
        games.iter().filter(|row| row.win).count() as f64 / n as f64
    } else {
        0.0
    };
    let overall_share = if n > 0 {
        games.iter().map(|row| row.score_share).sum::<f64>() / n as f64
    } else {
        0.0
    };
    // One game is one cluster: `(seed, pair·2 + arm)` — a classic row is alone
    // in its cluster, an all-seats game's rows share one.
    let game_key = |row: &Row| (row.seed, row.pair * 2 + usize::from(row.arm.min(1)));
    let bits: Vec<Vec<bool>> = games
        .iter()
        .map(|row| row.genome.chars().map(|c| c == '1').collect())
        .collect();

    // Adjusted: the pairs whose arms both exist, differenced.
    let complete = complete_pairs(rows);
    let mut diff_signs: Vec<Vec<f64>> = Vec::with_capacity(complete.len());
    let mut win_diffs = Vec::with_capacity(complete.len());
    let mut share_diffs = Vec::with_capacity(complete.len());
    let mut clusters = Vec::with_capacity(complete.len());
    for (a, b) in &complete {
        let (Some(sa), Some(sb)) = (
            pair_signs(&a.genome, &screened, k),
            pair_signs(&b.genome, &screened, k),
        ) else {
            continue;
        };
        // (±1 − ±1) / 2 ∈ {−1, 0, +1}: on in arm 0 only, same, on in arm 1 only.
        diff_signs.push(sa.iter().zip(&sb).map(|(x, y)| (x - y) / 2.0).collect());
        win_diffs.push(f64::from(u8::from(a.win)) - f64::from(u8::from(b.win)));
        share_diffs.push(a.score_share - b.score_share);
        clusters.push((a.seed, a.pair));
    }
    let adjusted = adjusted_effects(&diff_signs, &win_diffs, &clusters);

    let mut estimates = Vec::new();
    let mut column = 0;
    for (i, tag) in header.genes.iter().enumerate() {
        if !screened[i] {
            continue;
        }
        let this_column = column;
        column += 1;
        let (mut on_win, mut on_share, mut on_keys) = (Vec::new(), Vec::new(), Vec::new());
        let (mut off_win, mut off_share, mut off_keys) = (Vec::new(), Vec::new(), Vec::new());
        for (row, genome) in games.iter().zip(&bits) {
            let (win, share, keys) = if genome[i] {
                (&mut on_win, &mut on_share, &mut on_keys)
            } else {
                (&mut off_win, &mut off_share, &mut off_keys)
            };
            win.push(f64::from(u8::from(row.win)));
            share.push(row.score_share);
            keys.push(game_key(row));
        }
        let (win_on, win_on_se) = clustered_mean_se(&on_win, &on_keys);
        let (win_off, win_off_se) = clustered_mean_se(&off_win, &off_keys);
        let (share_on, share_on_se) = clustered_mean_se(&on_share, &on_keys);
        let (share_off, share_off_se) = clustered_mean_se(&off_share, &off_keys);
        let hypot = |a: f64, b: f64| (a * a + b * b).sqrt();
        estimates.push(GeneEstimate {
            tag: tag.clone(),
            pairs: complete.len(),
            n_on: on_win.len(),
            n_off: off_win.len(),
            win_on: if on_win.is_empty() { 0.0 } else { win_on },
            win_off: if off_win.is_empty() { 0.0 } else { win_off },
            win_delta: win_on - win_off,
            win_se: hypot(win_on_se, win_off_se),
            share_delta: share_on - share_off,
            share_se: hypot(share_on_se, share_off_se),
            adjusted: adjusted.as_ref().map(|all| all[this_column]),
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

/// One two-factor interaction between screened genes.
#[derive(Clone, Debug)]
struct Interaction {
    a: usize,
    b: usize,
    /// How much more gene `b` is worth when gene `a` is on (and symmetrically),
    /// on the outcome's own scale. `4γ` in the ±1 parameterisation.
    synergy: f64,
    se: f64,
}

impl Interaction {
    fn z(&self) -> f64 {
        if self.se > 0.0 && self.se.is_finite() {
            self.synergy / self.se
        } else {
            0.0
        }
    }
}

/// Every two-factor interaction, estimated from the PAIR SUMS.
///
/// ★★★★ THE FOLDOVER SPLITS THE EVIDENCE IN TWO AND THE MAIN TABLE USES ONLY
/// HALF OF IT. Write the outcome as `y = μ + Σβᵢxᵢ + Σγᵢⱼxᵢxⱼ` with
/// `x ∈ {−1,+1}`. The second arm of a pair is the exact complement, so every
/// `xᵢ` flips sign, and therefore:
///
/// - the **difference** `y(g) − y(ḡ)` keeps `2βᵢxᵢ` and CANCELS every
///   two-factor term, because `xᵢxⱼ − (−xᵢ)(−xⱼ) = 0`. That cancellation is
///   exactly why the main-effect table above is clean — a foldover de-aliases
///   main effects from two-factor interactions, which is the classical reason
///   to run one.
/// - the **sum** `y(g) + y(ḡ)` cancels every main effect and keeps `2γᵢⱼxᵢxⱼ`.
///
/// So the interactions were never missing from these games; they were in the
/// half of each pair the difference throws away. Nothing here needs a game
/// replayed.
///
/// Each `γᵢⱼ` is estimated marginally — `mean(centred pair sum × xᵢxⱼ) / 2` —
/// rather than jointly, because 57 genes have 1,596 two-factor terms and no
/// affordable run fits them all at once. The other terms are orthogonal in
/// expectation (the draws are independent coin flips), so a marginal estimate
/// is unbiased; what it pays is variance, since every other interaction and
/// the map's own difficulty sit in the residual. The reported figure is `4γ`:
/// **how much more one gene is worth when the other is on**.
///
/// ⚠ The map effect does NOT cancel here the way it does in the difference. A
/// pair's sum is twice its map mean plus the interaction terms, so these
/// estimates carry the full between-map variance and are far noisier than the
/// main effects from the same run. Read the multiplicity bar, not the top row.
fn interactions(
    header: &Header,
    rows: &[Row],
    outcome: fn(&Row) -> f64,
) -> (Vec<Interaction>, usize) {
    let k = header.genes.len();
    let screened = screened_mask(header);
    let complete = complete_pairs(rows);
    let mut signs: Vec<Vec<f64>> = Vec::with_capacity(complete.len());
    let mut sums: Vec<f64> = Vec::with_capacity(complete.len());
    for (a, b) in &complete {
        let Some(row_signs) = pair_signs(&a.genome, &screened, k) else {
            continue;
        };
        if b.genome.chars().count() != k {
            continue;
        }
        signs.push(row_signs);
        sums.push(outcome(a) + outcome(b));
    }
    let n = sums.len();
    if n < 3 {
        return (Vec::new(), n);
    }
    // Centre the sums: a pair's sum is dominated by its own map's difficulty,
    // which is a constant within the pair and contributes nothing to any
    // product's expectation once the mean is removed.
    let mean: f64 = sums.iter().sum::<f64>() / n as f64;
    let centred: Vec<f64> = sums.iter().map(|value| value - mean).collect();
    let width = signs[0].len();
    let mut found = Vec::with_capacity(width * width / 2);
    for a in 0..width {
        for b in a + 1..width {
            let products: Vec<f64> = signs
                .iter()
                .zip(&centred)
                .map(|(row, value)| row[a] * row[b] * value)
                .collect();
            let (mean, se) = mean_se(&products);
            // mean = 2γ, and the reported synergy is 4γ.
            found.push(Interaction {
                a,
                b,
                synergy: 2.0 * mean,
                se: 2.0 * se,
            });
        }
    }
    (found, n)
}

/// Print the strongest two-factor interactions on one outcome.
fn print_interactions(
    header: &Header,
    rows: &[Row],
    label: &str,
    scale: f64,
    unit: &str,
    outcome: fn(&Row) -> f64,
    top: usize,
) {
    let (mut found, pairs) = interactions(header, rows, outcome);
    if found.is_empty() {
        println!("\ninteractions ({label}): not enough complete pairs");
        return;
    }
    let names: Vec<&String> = header
        .genes
        .iter()
        .zip(screened_mask(header))
        .filter(|(_, screened)| *screened)
        .map(|(gene, _)| gene)
        .collect();
    let tests = found.len();
    let family_z = normal_quantile_upper(0.025 / tests as f64);
    found.sort_by(|a, b| b.z().abs().total_cmp(&a.z().abs()));
    let flagged = found.iter().filter(|row| row.z().abs() >= family_z).count();
    // ⚠ THE COUNT AT |z|≥2 IS THE ONLY HONEST HEADLINE HERE, and it is a
    // count against an expectation, not a list of exciting rows. 1,596 tests
    // throw ~73 flags at |z|≥2 with nothing whatever going on, so a table that
    // printed its top twelve without this line would read as twelve findings
    // every single time it was run — including on pure noise.
    let loose = found.iter().filter(|row| row.z().abs() >= 2.0).count();
    let expected_loose = tests as f64 * 0.0455;
    println!(
        "\ntwo-factor interactions on {label} · {pairs} pairs · {tests} gene pairs tested · \
         {loose} at |z|≥2 against {expected_loose:.0} expected by chance · \
         {flagged} past the family-wise bar |z|≥{family_z:.2} against {:.2} expected{}",
        0.05,
        if loose as f64 <= expected_loose && flagged == 0 {
            " ⇒ THIS LAYER IS INDISTINGUISHABLE FROM NOISE at this size; the rows below are the loudest noise, not findings"
        } else {
            ""
        }
    );
    println!(
        "estimated from the pair SUMS, which is the half of each pair the main-effect table cancels; \
         the figure is how much more one gene is worth when the other is on"
    );
    println!(
        "\n{:<28} {:<28} {:>10} {:>8} {:>7}  read",
        "gene", "with", "synergy", "±se", "z"
    );
    for row in found.iter().take(top) {
        let z = row.z();
        let read = if z.abs() >= family_z {
            if z > 0.0 {
                "SYNERGY **"
            } else {
                "ANTAGONISM **"
            }
        } else if z.abs() >= 2.0 {
            if z > 0.0 {
                "synergy *"
            } else {
                "antagonism *"
            }
        } else {
            "~"
        };
        println!(
            "{:<28} {:<28} {:>+9.2}{unit} {:>7.2} {:>+7.2}  {}",
            names[row.a],
            names[row.b],
            scale * row.synergy,
            scale * row.se,
            z,
            read
        );
    }
    if tests > top {
        println!("… {} weaker pairs not shown (--top N)", tests - top);
    }
}

fn print_table(header: &Header, rows: &[Row]) {
    let (mut estimates, pairs, overall_win, overall_share) = estimate(header, rows);
    let anchors: Vec<&Row> = rows.iter().filter(|row| row.kind == "anchor").collect();
    let game_pairs = if header.all_seats {
        let mut keys: Vec<(u64, usize)> = rows
            .iter()
            .filter(|row| row.kind == "game")
            .map(|row| (row.seed, row.pair))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys.len()
    } else {
        pairs
    };
    println!(
        "\ngene screen · {} complete pairs ({} games{}) · {}p {}x{} {} · {} · {} turns · {} city-states · baseline {} · field {} · {} · {}",
        pairs,
        game_pairs * 2,
        if header.all_seats {
            ", every major treated — errors clustered by game pair"
        } else {
            ""
        },
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
        },
        if header.victories.is_empty()
            || header.victories.split(',').count() == civvis::game::VictoryConditions::NAMES.len()
        {
            "all lanes".to_string()
        } else {
            format!("lanes {}", header.victories)
        }
    );
    println!(
        "treated seat overall: win {:.1}% (chance {:.1}%) · score share {:.1}% (equal share {:.1}%)",
        100.0 * overall_win,
        100.0 / header.players as f64,
        100.0 * overall_share,
        100.0 / header.players as f64
    );
    if header.design == "prior" {
        let screened = screened_mask(header);
        let mut by_p: BTreeMap<String, usize> = BTreeMap::new();
        for (p, &s) in header.prior.iter().zip(&screened) {
            if s {
                *by_p.entry(format!("{:.2}", p)).or_default() += 1;
            }
        }
        println!(
            "design: prior-weighted — each arm drawn independently from the ledger's prior \
             (genes at p = {}); Δ is the marginal on-versus-off contrast, errors clustered by \
             game; adjΔpp is the map-paired OLS on the arms' differences",
            by_p.iter()
                .map(|(p, n)| format!("{p}×{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
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
        // Paired: both arms of an anchor pair share the map and seat, so the
        // win difference carries its own standard error over pairs.
        let mut by_key: BTreeMap<(u64, usize, usize), [Option<&Row>; 2]> = BTreeMap::new();
        for row in &anchors {
            by_key
                .entry((row.seed, row.seat, row.pair))
                .or_insert([None, None])[usize::from(row.arm.min(1))] = Some(row);
        }
        let diffs: Vec<f64> = by_key
            .values()
            .filter_map(|[a, b]| {
                Some(f64::from(u8::from((*a)?.win)) - f64::from(u8::from((*b)?.win)))
            })
            .collect();
        let (delta, se) = mean_se(&diffs);
        // Under the deployment baseline arm 1 is the best genome, not all-off.
        let off_label = if header.baseline == "best" {
            "best genome"
        } else {
            "all-off"
        };
        println!(
            "anchors: all-on {} games win {:.1}% share {:.1}% · {off_label} {} games win {:.1}% share {:.1}% · paired win Δ {:+.1} pp ± {:.1} over {} pairs",
            on.len(),
            100.0 * rate(&on),
            100.0 * share(&on),
            off.len(),
            100.0 * rate(&off),
            100.0 * share(&off),
            100.0 * delta,
            100.0 * se,
            diffs.len()
        );
    }
    {
        // The religion census: how the treated seat stood in the race that
        // decides two thirds of these games. Printed only when the rows carry
        // it, so a file written before the instrumentation still analyses.
        let played: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == "game" || row.kind == "anchor")
            .collect();
        let instrumented = played
            .iter()
            .any(|row| row.founded_religion || row.faith > 0.0 || row.techs > 0);
        if instrumented && !played.is_empty() {
            let n = played.len() as f64;
            let founded = played.iter().filter(|row| row.founded_religion).count();
            let lost_to_faith: Vec<&&Row> = played
                .iter()
                .filter(|row| !row.win && row.victory == "religious")
                .collect();
            let mean = |rows: &[&&Row], f: fn(&Row) -> f64| {
                if rows.is_empty() {
                    0.0
                } else {
                    rows.iter().map(|row| f(row)).sum::<f64>() / rows.len() as f64
                }
            };
            println!(
                "religion census: founded a faith in {:.0}% of games · inquisition launched in {:.0}% · \
                 own cities under a foreign faith at the end {:.1} of {:.1} · faith left banked {:.0}",
                100.0 * founded as f64 / n,
                100.0 * played.iter().filter(|row| row.inquisition).count() as f64 / n,
                played.iter().map(|row| row.foreign_faith_cities as f64).sum::<f64>() / n,
                played.iter().map(|row| row.cities as f64).sum::<f64>() / n,
                played.iter().map(|row| row.faith).sum::<f64>() / n,
            );
            if !lost_to_faith.is_empty() {
                println!(
                    "  of the {} games lost to a rival's religion: {:.0}% had founded one of our own, \
                     {:.1} of {:.1} cities had flipped, {:.0} faith was still banked",
                    lost_to_faith.len(),
                    100.0
                        * lost_to_faith
                            .iter()
                            .filter(|row| row.founded_religion)
                            .count() as f64
                        / lost_to_faith.len() as f64,
                    mean(&lost_to_faith, |row| row.foreign_faith_cities as f64),
                    mean(&lost_to_faith, |row| row.cities as f64),
                    mean(&lost_to_faith, |row| row.faith),
                );
            }
        }
    }
    if estimates.is_empty() {
        println!("no screened genes with complete pairs");
        return;
    }
    let tranches = reproducibility_tranches(header, rows, REPRO_WINDOW_PAIRS, REPRO_WINDOW_COUNT);
    let tranche_sizes = tranches
        .iter()
        .enumerate()
        .map(|(index, tranche)| {
            let label = match index {
                0 => "latest",
                1 => "previous",
                _ => "earlier",
            };
            format!("{label}={}", tranche.pairs)
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "reproducibility windows (newest first): {tranche_sizes} complete paired comparisons; \
         target 10,000 each, rounded only to keep every all-seats game pair whole"
    );
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
    let prior_design = header.design == "prior";
    println!(
        "\n{:<28} {:>11} {:>6} {:>6} {:>16} {:>16} {:>16} {:>15} {:>6}  {:>8} {:>6}  {:>9}  read",
        "gene",
        if prior_design { "on n/off n" } else { "pairs" },
        "on%",
        "off%",
        "latest 10k",
        "prior 10k",
        "earlier 10k",
        "all 95% CI",
        "z",
        "shareΔ",
        "z",
        "adjΔpp"
    );
    for e in &estimates {
        let z = e.win_z();
        let read = read_column(z, e.share_z(), family_z);
        let adjusted = match e.adjusted {
            Some((effect, se)) => format!("{:+.1}±{:.1}", 100.0 * effect, 100.0 * se),
            None => "-".to_string(),
        };
        let count = if prior_design {
            format!("{}/{}", e.n_on, e.n_off)
        } else {
            e.pairs.to_string()
        };
        let latest = tranche_cell(tranches.first(), &e.tag);
        let previous = tranche_cell(tranches.get(1), &e.tag);
        let earlier = tranche_cell(tranches.get(2), &e.tag);
        println!(
            "{:<28} {:>11} {:>5.1}% {:>5.1}% {:>16} {:>16} {:>16} [{:>+6.1},{:>+6.1}] {:>+6.2}  {:>+7.2}pp {:>+6.2}  {:>9}  {}",
            e.tag,
            count,
            100.0 * e.win_on,
            100.0 * e.win_off,
            latest,
            previous,
            earlier,
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
         this size, NOT no effect. Each 10k cell is that window's win Δpp / paired z; on% minus \
         off% is the pooled win Δ. shareΔ is the score-share Δ in points; adjΔpp is the OLS win Δ \
         over the whole sign matrix."
    );
}

/// One gene's paired contrast split by the civilization the treated seat
/// played. Both arms of a pair share the civ (the roster shuffle is seeded by
/// the map seed), so every per-civ row is still a clean foldover contrast;
/// errors cluster by game pair as everywhere else. This is the subgroup the
/// marginal table averages away: a flag can be worth nothing on average and
/// still be a real strategy for one civilization — or the reverse.
fn print_by_civ(header: &Header, rows: &[Row], tag: &str) {
    let Some(index) = header.genes.iter().position(|gene| gene == tag) else {
        println!("\nby-civ: {tag:?} is not a gene in this file's header");
        return;
    };
    if !header.screened.iter().any(|gene| gene == tag) {
        println!("\nby-civ: {tag:?} was not screened in this file");
        return;
    }
    let complete = complete_pairs(rows);
    /// One civilization's evidence for the gene: paired win differences,
    /// paired share differences, and the game pair each came from.
    #[derive(Default)]
    struct CivEvidence {
        win: Vec<f64>,
        share: Vec<f64>,
        clusters: Vec<(u64, usize)>,
    }
    let mut by_civ: BTreeMap<&str, CivEvidence> = BTreeMap::new();
    let mut unlabelled = 0usize;
    for (a, b) in &complete {
        let bits: Vec<bool> = a.genome.chars().map(|c| c == '1').collect();
        if bits.len() != header.genes.len() {
            continue;
        }
        if a.civ.is_empty() {
            unlabelled += 1;
            continue;
        }
        let sign = if bits[index] { 1.0 } else { -1.0 };
        let entry = by_civ.entry(a.civ.as_str()).or_default();
        entry
            .win
            .push(sign * (f64::from(u8::from(a.win)) - f64::from(u8::from(b.win))));
        entry.share.push(sign * (a.score_share - b.score_share));
        entry.clusters.push((a.seed, a.pair));
    }
    if unlabelled > 0 {
        println!(
            "\nby-civ: {unlabelled} pairs have no civ on their rows (written before the field              existed) and are left out"
        );
    }
    if by_civ.is_empty() {
        println!("\nby-civ: no labelled pairs");
        return;
    }
    let civs = by_civ.len();
    let family_z = normal_quantile_upper(0.025 / civs as f64);
    println!(
        "\n{tag} by civilization · {} civs · family-wise 5% bar |z|≥{family_z:.2} (a subgroup scan: treat a flag as where to point a run, not a finding)",
        civs
    );
    println!(
        "{:<16} {:>6} {:>7} {:>15} {:>6}  {:>8} {:>6}  read",
        "civ", "pairs", "Δpp", "95% CI", "z", "shareΔ", "z"
    );
    let mut table: Vec<(&str, usize, f64, f64, f64, f64)> = by_civ
        .iter()
        .map(|(civ, evidence)| {
            let (wd, wse) = clustered_mean_se(&evidence.win, &evidence.clusters);
            let (sd, sse) = clustered_mean_se(&evidence.share, &evidence.clusters);
            (
                *civ,
                evidence.win.len(),
                wd,
                wse,
                sd,
                if sse > 0.0 && sse.is_finite() {
                    sd / sse
                } else {
                    0.0
                },
            )
        })
        .collect();
    table.sort_by(|a, b| {
        let za = if a.3 > 0.0 { a.2 / a.3 } else { 0.0 };
        let zb = if b.3 > 0.0 { b.2 / b.3 } else { 0.0 };
        zb.total_cmp(&za)
    });
    for (civ, pairs, wd, wse, sd, sz) in table {
        let z = if wse > 0.0 && wse.is_finite() {
            wd / wse
        } else {
            0.0
        };
        println!(
            "{:<16} {:>6} {:>+7.1} [{:>+6.1},{:>+6.1}] {:>+6.2}  {:>+7.2}pp {:>+6.2}  {}",
            civ,
            pairs,
            100.0 * wd,
            100.0 * (wd - 1.96 * wse),
            100.0 * (wd + 1.96 * wse),
            z,
            100.0 * sd,
            sz,
            read_column(z, sz, family_z)
        );
    }
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
                                || first.victories != found.victories
                                || first.all_seats != found.all_seats
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
         [--genes tag,tag,...] [--baseline best|repairs|stock] [--field advanced|repairs] \
         [--design foldover|prior] [--p-helps 0.9] [--p-hurts 0.1] [--p-unresolved 0.5] \
         [--anchor-pairs N] [--randomize-civs] [--all-seats] [--out PATH] [--append] [--quiet]\n       \
         gene_screen --analyze PATH [PATH ...] [--json OUT] [--interactions] [--top N] [--by-civ TAG]\n       \
         gene_screen --list"
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
        let weights = PriorWeights {
            helps: 0.9,
            hurts: 0.1,
            unresolved: 0.5,
        };
        println!(
            "{} genes (bit order) · default = the deployment genome (docs/gene_ledger.json) · \
             prior = on-probability under --design prior",
            genes.len()
        );
        for (i, gene) in genes.iter().enumerate() {
            let verdict = civvis::ai::ledger_verdict(gene.tag)
                .map(|row| row.verdict.as_str())
                .unwrap_or("unmeasured");
            println!(
                "{i:>3}  {:<28} {:<32} universe:{} stock:{} default:{} ledger:{:<10} prior:{:.1}",
                gene.tag,
                gene.field,
                if gene.after_setup_on { "on " } else { "off" },
                if gene.stock_on { "on " } else { "off" },
                if gene.default_on { "on " } else { "off" },
                verdict,
                weights.for_tag(gene.tag)
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
        if let Some(path) = text(&args, "--json") {
            write_json_summary(&path, &header, &rows);
        }
        if let Some(tag) = text(&args, "--by-civ") {
            print_by_civ(&header, &rows, &tag);
        }
        if present(&args, "--interactions") {
            let top = number(&args, "--top", 20).max(1) as usize;
            print_interactions(
                &header,
                &rows,
                "win rate",
                100.0,
                "pp",
                |row| f64::from(u8::from(row.win)),
                top,
            );
            print_interactions(
                &header,
                &rows,
                "score share",
                100.0,
                "pp",
                |row| row.score_share,
                top,
            );
        }
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
    // Every major seat carries its own drawn genome; arm 1 complements all of
    // them, so each gene is still on in exactly one arm of every seat's pair
    // and each game yields `players` observations instead of one. Outcomes
    // within one game share a winner, so the analysis clusters by game pair —
    // the gain over the classic design is real but less than ×players on the
    // win axis. The field is the other treated majors: effects are averaged
    // over random opposing genomes, not measured against a fixed production
    // field, so `--field` only shapes the anchors in this mode.
    let all_seats = present(&args, "--all-seats");
    // ⚠ THE REGIME DECIDES WHICH GENES CAN EVEN ACT. The first run's own
    // census: 66% of native 4-player games ended by RELIGIOUS conversion at a
    // median of turn 149, a third of them before turn 150 — so the thirty-one
    // war and siege genes were being asked what they contribute to a game that
    // was over before a siege could matter, and duly measured ~0. Restricting
    // the lanes is how a war repair gets a regime that lets a war happen
    // (`--victories domination,score`), and it is the same flag `civvis` itself
    // takes, parsed by the same function, so the two agree by construction.
    let victories = match text(&args, "--victories") {
        None => civvis::game::VictoryConditions::default(),
        Some(list) => civvis::game::VictoryConditions::parse(&list).unwrap_or_else(|why| {
            eprintln!(
                "--victories: {why}; choose from {:?}",
                civvis::game::VictoryConditions::NAMES
            );
            std::process::exit(2);
        }),
    };
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
        None | Some("best") => Baseline::Best,
        Some("repairs") => Baseline::Repairs,
        Some("stock") => Baseline::Stock,
        Some(other) => {
            eprintln!("unknown --baseline {other:?}; use best|repairs|stock");
            std::process::exit(2);
        }
    };
    let design = match text(&args, "--design").as_deref() {
        None | Some("foldover") => Design::Foldover,
        Some("prior") => Design::Prior,
        Some(other) => {
            eprintln!("unknown --design {other:?}; use foldover|prior");
            std::process::exit(2);
        }
    };
    let prior_weights = PriorWeights {
        helps: text(&args, "--p-helps")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.9),
        hurts: text(&args, "--p-hurts")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.1),
        unresolved: text(&args, "--p-unresolved")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5),
    };
    for (name, p) in [
        ("--p-helps", prior_weights.helps),
        ("--p-hurts", prior_weights.hurts),
        ("--p-unresolved", prior_weights.unresolved),
    ] {
        if !(0.0..=1.0).contains(&p) {
            eprintln!("{name} must be a probability, got {p}");
            std::process::exit(2);
        }
    }
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
    // The on-probability of every gene in header order: the prior for a
    // screened gene, the held state (0 or 1) for the rest. Recorded in the
    // header so `--analyze` can say what the draw was.
    let prior: Vec<f64> = match design {
        Design::Foldover => Vec::new(),
        Design::Prior => genes
            .iter()
            .zip(&screened)
            .map(|(gene, &s)| {
                if s {
                    prior_weights.for_tag(gene.tag)
                } else {
                    let held = match baseline {
                        Baseline::Best => gene.default_on,
                        Baseline::Repairs => gene.after_setup_on,
                        Baseline::Stock => gene.stock_on,
                    };
                    if held {
                        1.0
                    } else {
                        0.0
                    }
                }
            })
            .collect(),
    };

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
        victories: civvis::game::VictoryConditions::NAMES
            .iter()
            .filter(|name| victories.is_enabled(name))
            .copied()
            .collect::<Vec<_>>()
            .join(","),
        all_seats,
        design: design.id().to_string(),
        prior: prior.clone(),
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
        victories,
    };
    println!(
        "gene screen: {pairs} {} pairs ({} games{}){} · {} of {} genes screened · {players}p {width}x{height} {} · {} · {turns} turns · {city_states} city-states · {} civs · baseline {:?} · field {:?} · seeds {start_seed}..{} · {jobs} jobs · rows → {out_path}",
        design.id(),
        pairs * 2,
        if all_seats {
            format!(", every major treated: {} seat pairs", pairs * players)
        } else {
            String::new()
        },
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
    // Anchors: arm 0 every screened gene on, arm 1 every screened gene off —
    // or, at the deployment baseline, arm 1 at the ledger's defaults, so the
    // anchor prices the all-on universe against the best genome: what the
    // ledger bought.
    let all_on: Vec<bool> = genes
        .iter()
        .zip(&screened)
        .map(|(gene, &s)| if s { true } else { gene.after_setup_on })
        .collect();
    let all_off: Vec<bool> = genes
        .iter()
        .zip(&screened)
        .map(|(gene, &s)| {
            if s {
                baseline == Baseline::Best && gene.default_on
            } else {
                gene.after_setup_on
            }
        })
        .collect();
    let started = Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let wins = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(out);
    let games: Vec<Vec<Row>> = civvis::parallel::map_reporting(
        total_games,
        jobs,
        |index| {
            let pair = index / 2;
            let arm = (index % 2) as u8;
            let seed = start_seed + pair as u64;
            let seat = pair % players;
            if pair < pairs {
                if all_seats {
                    let genomes =
                        all_seat_genomes(start_seed, pair, players, &screened, arm, design, &prior);
                    return play_all_seats(
                        &profile, &genes, &screened, baseline, pair, arm, seed, &genomes,
                    );
                }
                let genome = match design {
                    Design::Foldover => {
                        let drawn = draw_genome(start_seed, pair, &screened);
                        if arm == 0 {
                            drawn
                        } else {
                            complement(&drawn, &screened)
                        }
                    }
                    Design::Prior => draw_genome_prior(start_seed, pair, arm, &screened, &prior),
                };
                return vec![play(
                    &profile, &genes, &screened, baseline, field, "game", pair, arm, seed, seat,
                    &genome,
                )];
            }
            // Anchors keep the classic single treated seat in both modes:
            // an all-on-versus-all-off contrast where every seat flips is
            // symmetric and measures nothing.
            let genome = if arm == 0 { &all_on } else { &all_off };
            vec![play(
                &profile, &genes, &screened, baseline, field, "anchor", pair, arm, seed, seat,
                genome,
            )]
        },
        |_index, game_rows| {
            let mut out = out.lock().expect("row writer");
            for row in game_rows {
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string(row).expect("row serializes")
                )
                .expect("write row");
                if row.win {
                    wins.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            out.flush().expect("flush rows");
            let finished = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if !quiet {
                let elapsed = started.elapsed().as_secs_f64();
                let row = &game_rows[0];
                println!(
                    "[{finished:>5}/{total_games}] {} pair {} arm {} seed {}{} · {} · t{} · {} · {:.0}s ({:.2} games/s, ~{:.0}s left)",
                    row.kind,
                    row.pair,
                    row.arm,
                    row.seed,
                    if game_rows.len() == 1 {
                        format!(" seat {}", row.seat)
                    } else {
                        format!(" ({} seats)", game_rows.len())
                    },
                    if row.victory.is_empty() { "-" } else { &row.victory },
                    row.turn,
                    if game_rows.len() == 1 {
                        format!(
                            "win={} share={:.1}% rank {} cities {}",
                            u8::from(row.win),
                            100.0 * row.score_share,
                            row.rank,
                            row.cities
                        )
                    } else {
                        match game_rows.iter().find(|r| r.win) {
                            Some(winner) => {
                                format!("winner seat {} ({})", winner.seat, winner.civ)
                            }
                            None => "no treated winner".to_string(),
                        }
                    },
                    row.secs,
                    finished as f64 / elapsed.max(1e-9),
                    elapsed / finished as f64 * (total_games - finished) as f64
                );
            }
        },
    );
    let rows: Vec<Row> = games.into_iter().flatten().collect();
    println!(
        "\n{total_games} games ({} rows) in {:.0}s ({:.2} games/s); treated seats won {} of them",
        rows.len(),
        started.elapsed().as_secs_f64(),
        total_games as f64 / started.elapsed().as_secs_f64().max(1e-9),
        wins.load(std::sync::atomic::Ordering::Relaxed)
    );
    print_table(&header, &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_header(genes: &[&str], all_seats: bool) -> Header {
        Header {
            kind: "header".into(),
            genes: genes.iter().map(|gene| (*gene).to_string()).collect(),
            screened: genes.iter().map(|gene| (*gene).to_string()).collect(),
            players: if all_seats { 3 } else { 4 },
            width: 1,
            height: 1,
            turns: 1,
            city_states: 0,
            speed: "online".into(),
            map: "pangaea".into(),
            baseline: "best".into(),
            field: "advanced".into(),
            start_seed: 1,
            randomize_civs: false,
            victories: String::new(),
            all_seats,
            design: "foldover".into(),
            prior: Vec::new(),
        }
    }

    fn test_row(pair: usize, seat: usize, arm: u8, genome: &str, win: bool) -> Row {
        Row {
            kind: "game".into(),
            pair,
            arm,
            seed: pair as u64,
            seat,
            genome: genome.into(),
            win,
            winner: None,
            victory: String::new(),
            turn: 1,
            score: 0,
            score_share: if win { 0.4 } else { 0.2 },
            rank: 1,
            cities: 0,
            alive: true,
            secs: 0.0,
            founded_religion: false,
            foreign_faith_cities: 0,
            faith: 0.0,
            inquisition: false,
            techs: 0,
            military: 0.0,
            civ: String::new(),
        }
    }

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
        // Firaxis-only flags are excluded by construction, not by luck —
        // unless a `PRODUCTION_OPT_INS` row names one on purpose. That table
        // is an author's statement that the flag acts on a native board
        // (`joint-tactics`: the bridge turns the joint search on, but
        // `advanced_joint_tactics` is production plus that flag and the
        // arena runs it every day); a repair reaches the genome only through
        // `ENGINE_REPAIR_TREATMENTS`, which still excludes every host-only tag.
        let opted_in: Vec<&str> = civvis::ai::PRODUCTION_OPT_INS
            .iter()
            .map(|(_, tag, _)| *tag)
            .collect();
        for gene in &genes {
            assert!(
                !civvis::elo::FIRAXIS_ONLY_TREATMENTS.contains(&gene.tag)
                    || opted_in.contains(&gene.tag),
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
        let mut on = [0usize; 8];
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

    /// The harmful `governor-every-lane` composite is deliberately split in
    /// the controller. These two rows make that split a real genome choice:
    /// a targeted screen can enable either established predicate while its
    /// sibling and the historical composite stay off.
    #[test]
    fn governor_halves_are_independent_opt_in_genes() {
        let genes = gene_table();
        let arm = |tag: &str| {
            let index = genes
                .iter()
                .position(|gene| gene.tag == tag)
                .unwrap_or_else(|| panic!("{tag} is a screenable gene"));
            let mut screened = vec![false; genes.len()];
            let mut genome = vec![false; genes.len()];
            screened[index] = true;
            genome[index] = true;
            treated_seat(&genes, &genome, &screened, Baseline::Stock)
        };

        let victory = arm("governor-victory-lanes");
        assert!(victory.governor_victory_lanes);
        assert!(!victory.governor_expansion_lane);
        assert!(!victory.governor_every_lane);

        let expansion = arm("governor-expansion-lane");
        assert!(!expansion.governor_victory_lanes);
        assert!(expansion.governor_expansion_lane);
        assert!(!expansion.governor_every_lane);
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
        let clusters: Vec<(u64, usize)> = (0..400).map(|pair| (pair as u64, pair)).collect();
        let fitted =
            adjusted_effects(&signs, &diffs, &clusters).expect("400 pairs support 5 genes");
        for (i, (effect, se)) in fitted.iter().enumerate() {
            assert!(
                (effect - planted[i]).abs() < 1e-9,
                "gene {i}: {effect} vs {}",
                planted[i]
            );
            assert!(*se < 1e-6);
        }
        assert!(
            adjusted_effects(&signs[..15], &diffs[..15], &clusters[..15]).is_none(),
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
            victories: String::new(),
            all_seats: false,
            design: "foldover".into(),
            prior: Vec::new(),
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
                    founded_religion: false,
                    foreign_faith_cities: 0,
                    faith: 0.0,
                    inquisition: false,
                    techs: 0,
                    military: 0.0,
                    civ: String::new(),
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

    /// A 10k tranche is a chronological replication, not an arbitrary slice
    /// of seat rows. This deliberately asks for four comparisons when one
    /// all-seats game contributes three: the nearest whole cluster is three,
    /// so a result from one map can never leak into two windows.
    #[test]
    fn reproducibility_tranches_are_newest_first_and_cluster_safe() {
        let header = test_header(&["a"], true);
        let mut rows = Vec::new();
        for (pair, on_wins) in [(0, false), (1, false), (2, false), (3, true)] {
            for seat in 0..3 {
                rows.push(test_row(pair, seat, 0, "1", on_wins));
                rows.push(test_row(pair, seat, 1, "0", !on_wins));
            }
        }

        let tranches = reproducibility_tranches(&header, &rows, 4, 3);
        assert_eq!(tranches.len(), 3);
        assert_eq!(
            tranches
                .iter()
                .map(|tranche| tranche.pairs)
                .collect::<Vec<_>>(),
            vec![3, 3, 3],
            "a four-pair target must not divide the three-seat game cluster"
        );
        let effect = |index: usize| tranches[index].estimates[0].win_delta;
        assert!((effect(0) - 1.0).abs() < 1e-9, "latest map is helpful");
        assert!((effect(1) + 1.0).abs() < 1e-9, "previous map is harmful");
        assert!((effect(2) + 1.0).abs() < 1e-9, "older map is harmful");
    }

    /// Synthetic rows with a planted interaction and planted main effects: the
    /// pair-sum estimator must find the interaction and must NOT report the
    /// main effects as interactions. That second half is the load-bearing one —
    /// it is the property that makes the foldover's two halves independent,
    /// and if it ever stops holding, every synergy in the table is really a
    /// main effect wearing a disguise.
    #[test]
    fn interactions_come_out_of_the_pair_sums_and_main_effects_do_not() {
        let genes: Vec<String> = ["a", "b", "c", "d"].iter().map(|g| g.to_string()).collect();
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
            victories: String::new(),
            all_seats: false,
            design: "foldover".into(),
            prior: Vec::new(),
        };
        // y = 0.5 + 0.30·a  −  0.20·c  + 0.25·(a·b) in the ±1 coding, plus a
        // per-map offset that is constant inside a pair — which is what a map
        // is. The design is the COMPLETE factorial (all 16 genomes of 4 genes,
        // each paired with its complement) repeated over ten maps, so every
        // sign product is exactly balanced and the recovery is exact rather
        // than approximate. A random draw would be off by an O(1/√n) term and
        // could only be asserted loosely, which would not prove the property.
        let screened = vec![true; 4];
        let mut rows = Vec::new();
        let mut pair = 0usize;
        for map in 0..10 {
            let map_offset = f64::from(map) * 0.05;
            for combination in 0u32..16 {
                let genome: Vec<bool> = (0..4).map(|bit| combination >> bit & 1 == 1).collect();
                let flipped = complement(&genome, &screened);
                for (arm, genome) in [(0u8, &genome), (1u8, &flipped)] {
                    let x: Vec<f64> = genome.iter().map(|&b| if b { 1.0 } else { -1.0 }).collect();
                    let y = 0.5 + 0.30 * x[0] - 0.20 * x[2] + 0.25 * x[0] * x[1] + map_offset;
                    rows.push(Row {
                        kind: "game".into(),
                        pair,
                        arm,
                        seed: pair as u64,
                        seat: pair % 4,
                        genome: genome_string(genome),
                        win: false,
                        winner: None,
                        victory: String::new(),
                        turn: 1,
                        score: 0,
                        score_share: y,
                        rank: 1,
                        cities: 0,
                        alive: true,
                        secs: 0.0,
                        founded_religion: false,
                        foreign_faith_cities: 0,
                        faith: 0.0,
                        inquisition: false,
                        techs: 0,
                        military: 0.0,
                        civ: String::new(),
                    });
                }
                pair += 1;
            }
        }
        let (found, pairs) = interactions(&header, &rows, |row| row.score_share);
        assert_eq!(pairs, 160);
        assert_eq!(found.len(), 6, "four genes have six two-factor pairs");
        let ab = found
            .iter()
            .find(|row| row.a == 0 && row.b == 1)
            .expect("a×b is a pair");
        // Planted γ = 0.25 in the ±1 coding, and the reported synergy is 4γ.
        assert!(
            (ab.synergy - 1.0).abs() < 1e-12,
            "planted a×b not recovered: {}",
            ab.synergy
        );
        for row in &found {
            if row.a == 0 && row.b == 1 {
                continue;
            }
            assert!(
                row.synergy.abs() < 1e-12,
                "genes {} and {} carry main effects but no interaction, and one leaked: {}",
                row.a,
                row.b,
                row.synergy
            );
        }
        // And the main-effect table still reads the main effects out of the
        // same rows: the two halves of a pair do not interfere.
        let (marginal, _, _, _) = estimate(&header, &rows);
        let a = marginal.iter().find(|e| e.tag == "a").unwrap();
        let b = marginal.iter().find(|e| e.tag == "b").unwrap();
        let c = marginal.iter().find(|e| e.tag == "c").unwrap();
        let d = marginal.iter().find(|e| e.tag == "d").unwrap();
        assert!((a.share_delta - 0.60).abs() < 1e-12, "a: {}", a.share_delta);
        assert!(
            b.share_delta.abs() < 1e-12,
            "b has no main effect, only an interaction, and the difference must not see it: {}",
            b.share_delta
        );
        assert!((c.share_delta + 0.40).abs() < 1e-12, "c: {}", c.share_delta);
        assert!(d.share_delta.abs() < 1e-9, "d is null: {}", d.share_delta);
    }

    /// All-seats mode: every seat draws its own genome, arm 1 complements
    /// every seat, and the draw reproduces from `(start_seed, pair, seat)`.
    #[test]
    fn all_seat_genomes_are_per_seat_foldovers() {
        let screened = vec![true, true, false, true];
        let a = all_seat_genomes(9, 4, 3, &screened, 0, Design::Foldover, &[]);
        let b = all_seat_genomes(9, 4, 3, &screened, 1, Design::Foldover, &[]);
        assert_eq!(a.len(), 3);
        assert_eq!(
            a,
            all_seat_genomes(9, 4, 3, &screened, 0, Design::Foldover, &[]),
            "reproduces"
        );
        for (ga, gb) in a.iter().zip(&b) {
            assert_eq!(
                ga,
                &complement(gb, &screened),
                "arm 1 complements every seat"
            );
            assert!(!ga[2] && !gb[2], "an un-screened gene is never drawn on");
        }
        assert_ne!(a[0], a[1], "seats draw independently (these seeds differ)");
    }

    /// Clustered errors: two seat pairs from one game whose differences
    /// cancel carry no evidence, and the estimator must say so. Treated as
    /// independent they would read as variance instead.
    #[test]
    fn clustered_mean_se_pools_seat_pairs_from_one_game() {
        // Two game pairs, two seats each; within every game the two seat
        // differences cancel exactly.
        let values = vec![1.0, -1.0, 1.0, -1.0];
        let clusters = vec![(7, 0), (7, 0), (8, 1), (8, 1)];
        let (mean, se) = clustered_mean_se(&values, &clusters);
        assert!(mean.abs() < 1e-12);
        assert!(se.abs() < 1e-12, "cluster means are both zero: {se}");
        // Singleton clusters reproduce mean_se exactly.
        let singles = vec![(1, 0), (2, 1), (3, 2), (4, 3)];
        let (m1, s1) = clustered_mean_se(&values, &singles);
        let (m2, s2) = mean_se(&values);
        assert!((m1 - m2).abs() < 1e-12 && (s1 - s2).abs() < 1e-12);
    }

    /// The estimate pipeline on an all-seats file: a planted gene is read at
    /// its value, and the standard error comes from game pairs, not from the
    /// (players × larger, correlated) seat-pair count.
    #[test]
    fn estimate_clusters_an_all_seats_file_by_game_pair() {
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
            victories: String::new(),
            all_seats: true,
            design: "foldover".into(),
            prior: Vec::new(),
        };
        let screened = vec![true, true];
        let mut rows = Vec::new();
        for pair in 0..200 {
            for seat in 0..4 {
                let g = draw_genome(13, pair * 4 + seat, &screened);
                let c = complement(&g, &screened);
                for (arm, genome) in [(0u8, &g), (1u8, &c)] {
                    rows.push(Row {
                        kind: "game".into(),
                        pair,
                        arm,
                        seed: pair as u64,
                        seat,
                        genome: genome_string(genome),
                        win: genome[0],
                        winner: None,
                        victory: String::new(),
                        turn: 1,
                        score: 0,
                        score_share: if genome[0] { 0.4 } else { 0.1 },
                        rank: 1,
                        cities: 0,
                        alive: true,
                        secs: 0.0,
                        founded_religion: false,
                        foreign_faith_cities: 0,
                        faith: 0.0,
                        inquisition: false,
                        techs: 0,
                        military: 0.0,
                        civ: ["rome", "egypt", "greece", "china"][seat].into(),
                    });
                }
            }
        }
        let (estimates, pairs, _, _) = estimate(&header, &rows);
        assert_eq!(pairs, 800, "seat pairs");
        let a = estimates.iter().find(|e| e.tag == "a").unwrap();
        assert!((a.win_delta - 1.0).abs() < 1e-9);
        // Every seat pair reads +1 for `a`, so its SE is zero either way.
        // `b` is null; its evidence comes from 200 cluster means, and the
        // point of this test is simply that the all-seats path pairs, keys
        // and estimates without losing the planted signal.
        let b = estimates.iter().find(|e| e.tag == "b").unwrap();
        assert!(b.win_delta.abs() < 0.2);
        assert!(b.win_se.is_finite() && b.win_se > 0.0);
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
            founded_religion: true,
            foreign_faith_cities: 2,
            faith: 310.5,
            inquisition: false,
            techs: 40,
            military: 820.0,
            civ: "rome".into(),
        };
        let text = serde_json::to_string(&row).unwrap();
        let back: Row = serde_json::from_str(&text).unwrap();
        assert_eq!(back.genome, "0110");
        assert_eq!(back.winner, Some(2));
        assert!(serde_json::from_str::<Header>(&text)
            .map(|h| h.kind != "header")
            .unwrap_or(true));
    }

    /// ★★★★ THE HELPFUL GENES PLAY MOST OF THE TIME. Under the prior design a
    /// gene is on with its own probability, the two arms of a pair are
    /// independent draws, and a run reproduces from its seed.
    #[test]
    fn prior_draws_follow_the_weights_and_arms_are_independent() {
        let screened = vec![true, true, true, false];
        let prior = vec![0.9, 0.1, 0.5, 1.0];
        let mut on = [0usize; 4];
        let mut differ = 0usize;
        let pairs = 4000;
        for pair in 0..pairs {
            let a = draw_genome_prior(5, pair, 0, &screened, &prior);
            let b = draw_genome_prior(5, pair, 1, &screened, &prior);
            assert_eq!(
                a,
                draw_genome_prior(5, pair, 0, &screened, &prior),
                "reproduces"
            );
            assert!(!a[3] && !b[3], "an un-screened gene is never drawn on");
            for (i, &bit) in a.iter().enumerate() {
                on[i] += usize::from(bit);
            }
            differ += usize::from(a[0] != b[0]);
        }
        let rate = |i: usize| on[i] as f64 / pairs as f64;
        assert!((rate(0) - 0.9).abs() < 0.03, "helper on ~90%: {}", rate(0));
        assert!((rate(1) - 0.1).abs() < 0.03, "harmful on ~10%: {}", rate(1));
        assert!(
            (rate(2) - 0.5).abs() < 0.03,
            "unresolved on ~50%: {}",
            rate(2)
        );
        // Independent arms differ on a p = 0.9 gene in 2·0.9·0.1 = 18% of pairs.
        let discordant = differ as f64 / pairs as f64;
        assert!(
            (discordant - 0.18).abs() < 0.03,
            "arms drawn independently: {discordant}"
        );
    }

    /// The prior weights come from the ledger's verdict per tag.
    #[test]
    fn prior_weights_follow_the_ledger() {
        let weights = PriorWeights {
            helps: 0.9,
            hurts: 0.1,
            unresolved: 0.5,
        };
        for row in civvis::ai::gene_ledger_rows() {
            let expected = match row.verdict {
                civvis::ai::Verdict::Helps => 0.9,
                civvis::ai::Verdict::Hurts => 0.1,
                civvis::ai::Verdict::Unresolved => 0.5,
            };
            assert_eq!(weights.for_tag(row.tag), expected, "{}", row.tag);
        }
        assert_eq!(
            weights.for_tag("no-such-gene"),
            0.5,
            "unmeasured draws at one half"
        );
    }

    /// Under an unbalanced draw the marginal contrast still reads a planted
    /// gene — the 90% against the 10% — and the map-paired OLS recovers it
    /// from the pairs whose arms differ on it.
    #[test]
    fn estimate_prior_reads_a_planted_gene_through_an_unbalanced_draw() {
        let genes = vec!["a".to_string(), "b".to_string()];
        let prior = vec![0.9, 0.5];
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
            baseline: "best".into(),
            field: "advanced".into(),
            start_seed: 1,
            randomize_civs: false,
            victories: String::new(),
            all_seats: false,
            design: "prior".into(),
            prior: prior.clone(),
        };
        let screened = vec![true, true];
        let mut rows = Vec::new();
        for pair in 0..600 {
            for arm in 0u8..2 {
                let genome = draw_genome_prior(3, pair, arm, &screened, &prior);
                // Gene `a` on wins; gene `b` does nothing; the map adds a
                // per-pair share offset that the paired OLS cancels.
                rows.push(Row {
                    kind: "game".into(),
                    pair,
                    arm,
                    seed: pair as u64,
                    seat: pair % 4,
                    genome: genome_string(&genome),
                    win: genome[0],
                    winner: None,
                    victory: String::new(),
                    turn: 1,
                    score: 0,
                    score_share: if genome[0] { 0.4 } else { 0.2 } + (pair % 5) as f64 * 0.01,
                    rank: 1,
                    cities: 0,
                    alive: true,
                    secs: 0.0,
                    founded_religion: false,
                    foreign_faith_cities: 0,
                    faith: 0.0,
                    inquisition: false,
                    techs: 0,
                    military: 0.0,
                    civ: String::new(),
                });
            }
        }
        let (estimates, pairs, overall_win, _) = estimate(&header, &rows);
        assert_eq!(pairs, 600);
        assert!(
            (overall_win - 0.9).abs() < 0.05,
            "a is on 90% of the time: {overall_win}"
        );
        let a = estimates.iter().find(|e| e.tag == "a").unwrap();
        let b = estimates.iter().find(|e| e.tag == "b").unwrap();
        assert!(
            a.n_on > 1000 && a.n_off < 200,
            "unbalanced: {}/{}",
            a.n_on,
            a.n_off
        );
        assert!((a.win_on - 1.0).abs() < 1e-9 && a.win_off.abs() < 1e-9);
        assert!(
            (a.win_delta - 1.0).abs() < 1e-9,
            "the 90% beat the 10% by the planted 100pp"
        );
        assert!((a.share_delta - 0.2).abs() < 0.02, "{}", a.share_delta);
        assert!(
            b.win_delta.abs() < 0.1,
            "b is planted null: {}",
            b.win_delta
        );
        let (adj_a, _) = a.adjusted.expect("600 pairs support 2 genes");
        assert!((adj_a - 1.0).abs() < 1e-6, "paired OLS recovers a: {adj_a}");
        let (adj_b, _) = b.adjusted.unwrap();
        assert!(adj_b.abs() < 0.1, "paired OLS reads b null: {adj_b}");
    }

    /// The deployment baseline holds an un-screened gene at the ledger's
    /// default: a measured harmful repair is off, a measured helper on.
    #[test]
    fn the_best_baseline_holds_unscreened_genes_at_the_ledger_default() {
        let genes = gene_table();
        let none = vec![false; genes.len()];
        let genome = vec![false; genes.len()];
        let best = treated_seat(&genes, &genome, &none, Baseline::Best);
        let siege = genes.iter().find(|g| g.tag == "siege-is-progress").unwrap();
        let reinforcement = genes.iter().find(|g| g.tag == "war-reinforcement").unwrap();
        assert_eq!(best.siege_is_progress, siege.default_on);
        assert_eq!(best.war_reinforcement, reinforcement.default_on);
        assert_eq!(
            siege.default_on,
            civvis::ai::ledger_default_on("siege-is-progress").unwrap()
        );
        assert_eq!(
            reinforcement.default_on,
            civvis::ai::ledger_default_on("war-reinforcement").unwrap()
        );
        // And the universe baseline keeps every repair on whatever the ledger says.
        let universe = treated_seat(&genes, &genome, &none, Baseline::Repairs);
        assert!(universe.siege_is_progress && universe.war_reinforcement);
    }
}
