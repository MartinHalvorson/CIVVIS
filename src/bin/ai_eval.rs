//! Paired, seat-balanced head-to-head evaluator for built-in AIs.
use civvis::ai::{Ai, ExpansionCensus, WarPlanReport};
use civvis::elo::{
    builtin_ai_degraded, builtin_ai_strict, builtin_arm, builtin_provenances, collapsed_entrants,
    AgentProvenance, BuiltinAiBuildError, ARTIFACT_DIR, BUILTIN_AIS, EVAL_ONLY_AIS,
};
use civvis::game::{default_difficulty, Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapTopology};
use civvis::strategic::ReviewCensus;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::Command;

/// Arms whose treatment can only act through city-states.
///
/// Listed so `--city-states 0`, which is the default, cannot silently turn one
/// of them into a null. Add an arm here when its axis needs a minor to exist.
const MINOR_DEPENDENT_ARMS: [&str; 8] = [
    "advanced_diplomatic_opening",
    "advanced_envoy_policy",
    "advanced_envoy_infrastructure",
    "advanced_envoy_priority",
    "advanced_envoy_economy",
    "advanced_policy_envoy_priority",
    // Added 2026-08-19. `SUZERAIN_PRIZE` is scored only inside the envoy
    // placement loop, over `g.can_send_envoy(pid, minor.id)`, so with no minor
    // seated the arm is byte-identical to its control: 12 pairs at the stock
    // 4p profile returned 0 favored / 12 neutral / 0 against on wins *and* on
    // terminal score. Both 400-pair runs that decided this flag ships off were
    // hand-rolled `ai_eval` lines, which is the path that defaults to zero.
    "advanced_price_suzerainty",
    // Added with the arm itself (#2185). It turns on `diplomatic_opening` and
    // `price_the_suzerainty` together, and both halves reach the board only
    // through a minor: the opening requires a met, unclaimed city-state and
    // the prize is scored inside the envoy placement loop.
    "advanced_diplomacy_lane",
];

/// Arms measured to complete so rarely at the deployment profile that a margin
/// against them measures THEIR floor rather than the other arm's strength.
///
/// ⚠⚠ THIS IS A CONTROL PROBLEM, AND IT HAS ALREADY PRODUCED A NUMBER NOBODY
/// SHOULD QUOTE. `victory_eval --players 6 --turns 250 --speed online`, 96
/// games on two disjoint seed streams, completes:
///
///   diplomatic 14/16 · culture 12/16 · religious 8/16 · domination 2/16 ·
///   **science 0/16**
///
/// Every named lane was then compared against `advanced_target_science` and all
/// four beat it — diplomatic by +669 Elo, "CONFIRMED", 23-0-1. Promoted effects
/// on this ledger run +30..+40, so a +669 is a broken incumbent rather than a
/// discovery, and the demonstration is in the ledger too: diplomatic against
/// religious, the fair fight between two lanes that BOTH finish, is 47.9%,
/// −14 Elo, p=1.0000, inconclusive. If Diplomacy were strong it would beat
/// Religion. It does not.
///
/// `EVAL_INTEGRITY.md` R1 names this family — "controls are not matched" — and
/// the repository has repaired the genome-matching instance of it already. This
/// is the same defect one level up: the control is the arm carrying less.
///
/// Listed rather than derived because "can this arm finish" is a measurement,
/// not something the binary can compute at startup. Add an arm here when a
/// screen shows it cannot complete at this shape, and cite the screen.
const DEGENERATE_CONTROLS: [(&str, &str); 1] = [(
    "advanced_target_science",
    "completes 0/16 at the deployment profile (victory_eval, 96 games, two disjoint streams)",
)];

use civvis::rng::Rng;

const PROMOTION_MIN_MAPS: usize = 20;
/// Opt-in early stopping. After every scheduling chunk the paired inference is
/// re-read, and the run ends as soon as the promotion gate is decisive —
/// `PASS` or `RETAIN` — instead of playing every preregistered pair.
///
/// This is statistically clean for the gate itself: its verdict rests on the
/// anytime-valid betting evidence and the betting interval, both of which hold
/// under optional stopping by construction (that is what "anytime-valid"
/// means), so a verdict read at map k is the same verdict the full run would
/// have been entitled to at map k. It is NOT clean for the two legacy readings
/// printed beside it — the exact sign test and the retired Wilson interval
/// assume a fixed sample — so a stopped run says so on its own report and
/// those lines are not confirmatory there.
///
/// Off by default, so every preregistered fixed-N run is exactly what it was.
/// The recorded crossings say what it buys: map 42, 46, 51, 57 and 134 on runs
/// of 120–400 pairs (`docs/EVAL.md`), i.e. 0–70% of an evaluation's wall.
const STOP_WHEN_DECISIVE: &str = "--stop-when-decisive";
const Z_95: f64 = 1.959_963_984_540_054;
/// Split a 5% two-sided error budget equally between promotion and retention.
const ANYTIME_TAIL_ALPHA: f64 = 0.025;
/// Fixed, pre-declared bets for a finite mixture e-process. At the parity null
/// every paired-map score is in [0, 1], so each factor
/// `1 + lambda * (score - 0.5)` is nonnegative and has expectation at most one
/// for the challenger-side test. Negating the bet tests the incumbent side.
const BET_LAMBDAS: [f64; 10] = [0.05, 0.10, 0.20, 0.35, 0.50, 0.70, 0.90, 1.15, 1.45, 1.80];
/// Candidate means scanned when the betting test is inverted into an interval.
/// The scan only has to isolate each endpoint's cell; bisection inside that
/// cell supplies the precision the report prints.
const INTERVAL_GRID: usize = 1_000;
/// Bisection steps per endpoint, which take a 1/1000 cell below 1e-9.
const INTERVAL_REFINEMENTS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromotionVerdict {
    Insufficient,
    Promote,
    Retain,
    Inconclusive,
}

#[derive(Debug, Clone, Copy)]
struct PairedInference {
    maps: usize,
    score: f64,
    /// Anytime-valid betting interval. This is the gate's interval.
    low: f64,
    high: f64,
    /// The maximum-variance Wilson interval this gate used to decide on, kept
    /// so every historical run stays comparable and the width the old rule
    /// charged is visible beside the width the evidence supports.
    wilson_low: f64,
    wilson_high: f64,
    elo: f64,
    elo_low: f64,
    elo_high: f64,
    anytime: AnytimeEvidence,
    verdict: PromotionVerdict,
}

#[derive(Debug, Clone, Copy)]
struct AnytimeEvidence {
    challenger_peak_e: f64,
    incumbent_peak_e: f64,
    challenger_p: f64,
    incumbent_p: f64,
    challenger_crossed_at: Option<usize>,
    incumbent_crossed_at: Option<usize>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PairOutcomes {
    a_sweeps: usize,
    neutral: usize,
    b_sweeps: usize,
    mixed_with_draw: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DirectionalOutcomes {
    challenger_favored: usize,
    neutral: usize,
    incumbent_favored: usize,
}

fn elo_edge(score: f64) -> f64 {
    let bounded = score.clamp(1e-6, 1.0 - 1e-6);
    400.0 * (bounded / (1.0 - bounded)).log10()
}

/// Who sits in each chair for one game of a pair, and which chairs the verdict
/// is computed from.
///
/// Fieldless — every recorded result in `docs/EVAL.md` — each chair alternates
/// between the entrants and `swap` moves the challenger around the whole map,
/// so start-position quality cancels across the pair. With a field the entrants
/// hold seats 0 and 1 and still swap, which is the same balancing argument over
/// two chairs instead of all of them, and the field cycles through the rest.
///
/// ⚠ The returned seat sets are the verdict's, not the telemetry's. Per-agent
/// metrics stay keyed by name, because two chairs holding the same agent should
/// pool their telemetry; the verdict must not, because a field agent is neither
/// entrant and its victory belongs to neither of them.
fn seat_plan<'a>(
    players: usize,
    swap: usize,
    a: &'a str,
    b: &'a str,
    field: &[&'a str],
) -> (Vec<&'a str>, BTreeSet<usize>, BTreeSet<usize>) {
    let entrant = |pid: usize| if (pid + swap).is_multiple_of(2) { a } else { b };
    let seats: Vec<&str> = (0..players)
        .map(|pid| {
            if field.is_empty() || pid < 2 {
                entrant(pid)
            } else {
                field[(pid - 2) % field.len()]
            }
        })
        .collect();
    let entrant_seats: Vec<usize> = if field.is_empty() {
        (0..players).collect()
    } else {
        (0..players.min(2)).collect()
    };
    let challenger_seats = entrant_seats
        .iter()
        .copied()
        .filter(|pid| seats[*pid] == a)
        .collect();
    let incumbent_seats = entrant_seats
        .iter()
        .copied()
        .filter(|pid| seats[*pid] == b)
        .collect();
    (seats, challenger_seats, incumbent_seats)
}

/// The pair's half-point for one game.
///
/// ⚠ Indexed by seat, not by name, because `--field` puts agents on the board
/// that are neither entrant. Naming the winner was enough while every chair
/// held `a` or `b`: anything that was not the challenger was the incumbent by
/// construction. With a field it is not, and the name test would have scored a
/// field seat's victory as a win *for the incumbent* — the one arrangement that
/// could make a denial treatment look worse precisely when it worked.
///
/// A game won by neither entrant is a draw, exactly as a game nobody won
/// already was: neither side achieved the objective. Fieldless, the two seat
/// sets partition every chair, so this is identical to the name test it
/// replaces.
fn game_score(
    winner: Option<usize>,
    challenger_seats: &BTreeSet<usize>,
    incumbent_seats: &BTreeSet<usize>,
) -> f64 {
    match winner {
        Some(pid) if challenger_seats.contains(&pid) => 1.0,
        Some(pid) if incumbent_seats.contains(&pid) => 0.0,
        _ => 0.5,
    }
}

/// Construct one evaluator seat through the same strict boundary that guards
/// the command-line preflight. `--allow-degraded` is intentionally the only
/// route to the explicitly named fallback factory.
fn evaluator_ai(
    name: &str,
    seed: u64,
    allow_degraded: bool,
) -> Result<Box<dyn Ai>, BuiltinAiBuildError> {
    if allow_degraded {
        Ok(builtin_ai_degraded(name, seed))
    } else {
        builtin_ai_strict(name, seed)
    }
}

/// Challenger share of terminal Civilization score across the evaluated
/// seats. This is a bounded secondary development diagnostic, not a win and
/// never an input to the promotion verdict.
fn terminal_score_share(
    g: &Game,
    challenger_seats: &BTreeSet<usize>,
    incumbent_seats: &BTreeSet<usize>,
) -> f64 {
    let mut challenger_score = 0_i64;
    let mut total_score = 0_i64;
    for pid in challenger_seats.iter().chain(incumbent_seats.iter()) {
        let score = g.score(*pid).max(0);
        total_score += score;
        if challenger_seats.contains(pid) {
            challenger_score += score;
        }
    }
    if total_score > 0 {
        challenger_score as f64 / total_score as f64
    } else {
        0.5
    }
}

fn log_mean_exp(values: &[f64]) -> f64 {
    let largest = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    largest
        + values
            .iter()
            .map(|value| (*value - largest).exp())
            .sum::<f64>()
            .ln()
        - (values.len() as f64).ln()
}

/// Peak evidence, in each direction, from a mixture of betting martingales
/// against the hypothesis that the true paired-map mean is `candidate`.
///
/// `BET_LAMBDAS` is declared against parity, where a map score's edge spans
/// +/- 0.5 and the largest safe stake is 2. A candidate mean `m` moves that
/// span to `[-m, 1 - m]`, so every stake is rescaled by `stake_cap(m) / 2`:
/// the shipped grid is reproduced exactly at `m == 0.5`, and no factor can go
/// nonpositive anywhere else.
///
/// One extra bet is **adaptive**: it stakes the growth-rate-optimal amount
/// implied by the running mean and variance. Ville's inequality needs only
/// that a stake be *predictable* — a function of the maps already seen — so
/// this stays exactly as valid as the fixed grid while concentrating capital
/// on the bet size the data actually supports. A uniform grid spends nine
/// tenths of its wealth on stakes the run has already ruled out.
struct BettingPeaks {
    challenger_log_e: f64,
    incumbent_log_e: f64,
    challenger_crossed_at: Option<usize>,
    incumbent_crossed_at: Option<usize>,
}

/// The largest stake that keeps `1 +/- lambda * (score - candidate)` positive
/// for every score in [0, 1]. Two at parity, and never below one.
fn stake_cap(candidate: f64) -> f64 {
    1.0 / candidate.max(1.0 - candidate).max(f64::MIN_POSITIVE)
}

fn betting_peaks(scores: &[f64], candidate: f64, monitor_from: usize) -> BettingPeaks {
    let cap = stake_cap(candidate);
    let scale = cap / 2.0;
    let mut challenger_log_wealth = [0.0; BET_LAMBDAS.len()];
    let mut incumbent_log_wealth = [0.0; BET_LAMBDAS.len()];
    let mut challenger_adaptive = 0.0_f64;
    let mut incumbent_adaptive = 0.0_f64;
    let mut challenger_peak = 0.0_f64;
    let mut incumbent_peak = 0.0_f64;
    let mut challenger_crossed_at = None;
    let mut incumbent_crossed_at = None;
    let crossing_log_e = -(ANYTIME_TAIL_ALPHA.ln());
    // Shrunk running moments, so the first map bets from parity rather than
    // from a degenerate one-sample estimate.
    let mut seen = 0.0_f64;
    let mut sum = 0.0_f64;
    let mut sum_squared_deviation = 0.0_f64;
    let mut mixture = [0.0; BET_LAMBDAS.len() + 1];

    for (index, raw_score) in scores.iter().enumerate() {
        debug_assert!((0.0..=1.0).contains(raw_score));
        let edge = raw_score.clamp(0.0, 1.0) - candidate;
        for (bet, lambda) in BET_LAMBDAS.iter().enumerate() {
            let stake = lambda * scale;
            challenger_log_wealth[bet] += (1.0 + stake * edge).ln();
            incumbent_log_wealth[bet] += (1.0 - stake * edge).ln();
        }
        // Predictable: every term below is fixed before this map is read.
        let running_mean = (0.5 + sum) / (1.0 + seen);
        let running_variance = (0.25 + sum_squared_deviation) / (1.0 + seen);
        let drift = running_mean - candidate;
        let optimal = drift / (running_variance + drift * drift);
        let adaptive_cap = 0.9 * cap;
        challenger_adaptive += (1.0 + optimal.clamp(0.0, adaptive_cap) * edge).ln();
        incumbent_adaptive += (1.0 - (-optimal).clamp(0.0, adaptive_cap) * edge).ln();
        seen += 1.0;
        sum += raw_score;
        let updated_mean = (0.5 + sum) / (1.0 + seen);
        sum_squared_deviation += (raw_score - updated_mean).powi(2);

        let maps = index + 1;
        if maps < monitor_from {
            continue;
        }
        mixture[..BET_LAMBDAS.len()].copy_from_slice(&challenger_log_wealth);
        mixture[BET_LAMBDAS.len()] = challenger_adaptive;
        let challenger_log_e = log_mean_exp(&mixture);
        mixture[..BET_LAMBDAS.len()].copy_from_slice(&incumbent_log_wealth);
        mixture[BET_LAMBDAS.len()] = incumbent_adaptive;
        let incumbent_log_e = log_mean_exp(&mixture);
        challenger_peak = challenger_peak.max(challenger_log_e);
        incumbent_peak = incumbent_peak.max(incumbent_log_e);
        if challenger_crossed_at.is_none() && challenger_log_e >= crossing_log_e {
            challenger_crossed_at = Some(maps);
        }
        if incumbent_crossed_at.is_none() && incumbent_log_e >= crossing_log_e {
            incumbent_crossed_at = Some(maps);
        }
    }

    BettingPeaks {
        challenger_log_e: challenger_peak,
        incumbent_log_e: incumbent_peak,
        challenger_crossed_at,
        incumbent_crossed_at,
    }
}

/// Invert the betting test into a confidence interval: keep every candidate
/// mean the run's own evidence cannot reject at `ANYTIME_TAIL_ALPHA` per side.
///
/// This is the interval the promotion gate needs and Wilson cannot supply.
/// Wilson charges a split map — the commonest outcome of a mirrored A/B, and
/// the one carrying no direction at all — the full `p(1 - p)` variance of a
/// coin flip. Real evaluator runs sit 3x to 6x under that, so the interval is
/// roughly twice as wide as the evidence warrants and a genuine edge reads as
/// inconclusive. A betting interval is finite-sample valid for *any* bounded
/// observation, so it narrows on concentrated runs without ever assuming the
/// dispersion it is measuring: it does not estimate a variance, it bets.
///
/// Monitoring begins at the same map the gate's evidence does, so at
/// `PROMOTION_MIN_MAPS` or more the interval excludes parity on exactly the
/// runs `anytime_evidence` calls decisive. Shorter runs are reported from
/// their final prefix and remain `Insufficient`.
fn betting_interval(scores: &[f64]) -> (f64, f64) {
    if scores.is_empty() {
        return (0.0, 1.0);
    }
    let monitor_from = PROMOTION_MIN_MAPS.min(scores.len());
    let crossing_log_e = -(ANYTIME_TAIL_ALPHA.ln());
    let retained = |candidate: f64| {
        let peaks = betting_peaks(scores, candidate, monitor_from);
        peaks.challenger_log_e < crossing_log_e && peaks.incumbent_log_e < crossing_log_e
    };
    // The stake rescaling makes each side's evidence very nearly monotone in
    // the candidate but not provably so, so isolate the endpoints by scan
    // rather than by assuming one crossing, then bisect for precision.
    let mut first = None;
    let mut last = None;
    for step in 0..=INTERVAL_GRID {
        let candidate = step as f64 / INTERVAL_GRID as f64;
        if retained(candidate) {
            first.get_or_insert(step);
            last = Some(step);
        }
    }
    let (Some(first), Some(last)) = (first, last) else {
        // Every candidate rejected: the run is its own best estimate.
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        return (mean, mean);
    };
    let cell = 1.0 / INTERVAL_GRID as f64;
    let refine = |mut rejected: f64, mut kept: f64| {
        for _ in 0..INTERVAL_REFINEMENTS {
            let middle = 0.5 * (rejected + kept);
            if retained(middle) {
                kept = middle;
            } else {
                rejected = middle;
            }
        }
        kept
    };
    let low = if first == 0 {
        0.0
    } else {
        refine(first as f64 * cell - cell, first as f64 * cell)
    };
    let high = if last == INTERVAL_GRID {
        1.0
    } else {
        refine(last as f64 * cell + cell, last as f64 * cell)
    };
    (low.clamp(0.0, 1.0), high.clamp(0.0, 1.0))
}

/// Anytime-valid evidence against parity from a finite mixture of betting
/// martingales. The process starts with one unit of wealth; Ville's inequality
/// makes `1 / peak wealth` a valid upper bound on the probability of ever
/// observing at least this much evidence under the null, even if the evaluator
/// is rerun with longer prefixes and stopped when a result looks favorable.
///
/// Monitoring begins only at `PROMOTION_MIN_MAPS`, so a lucky sub-minimum prefix
/// cannot earn a permanent promotion before the representativeness floor.
fn anytime_evidence(scores: &[f64]) -> AnytimeEvidence {
    let peaks = betting_peaks(scores, 0.5, PROMOTION_MIN_MAPS);
    AnytimeEvidence {
        challenger_peak_e: peaks.challenger_log_e.min(f64::MAX.ln()).exp(),
        incumbent_peak_e: peaks.incumbent_log_e.min(f64::MAX.ln()).exp(),
        challenger_p: (-peaks.challenger_log_e).exp().min(1.0),
        incumbent_p: (-peaks.incumbent_log_e).exp().min(1.0),
        challenger_crossed_at: peaks.challenger_crossed_at,
        incumbent_crossed_at: peaks.incumbent_crossed_at,
    }
}

/// Whether `STOP_WHEN_DECISIVE` may end the run here: the gate has read a
/// `PASS` or a `RETAIN` on the pairs played so far. `Insufficient` and
/// `Inconclusive` keep playing.
fn early_stop_is_warranted(scores: &[f64]) -> bool {
    matches!(
        paired_inference(scores).verdict,
        PromotionVerdict::Promote | PromotionVerdict::Retain
    )
}

/// Paired-map inference: the score, both intervals, and the promotion verdict.
///
/// Pair scores can be fractional because a split scores 0.5 and a game without
/// a winner is a draw. One mirrored map is one observation, so the swapped
/// games are never falsely counted as independent evidence.
///
/// ⚠ The interval that *decides* is `betting_interval`, not Wilson. Wilson
/// treats each map score as Bernoulli and so charges it the maximum variance
/// `p(1 - p)` for its mean. That is the right bound for a coin flip and the
/// wrong one for this design: a mirrored A/B between close agents splits most
/// of its maps, and a split is exactly the observation that carries no
/// dispersion at all. Measured on the runs this repository has actually
/// recorded, the empirical variance is 3.3x to 5.6x under the Bernoulli
/// bound, so the old interval ran about twice as wide as the evidence
/// warranted and rejected real edges as inconclusive. The betting interval is
/// finite-sample valid for any bounded observation and adapts to the
/// dispersion instead of assuming the worst of it. Both are reported.
fn paired_inference(scores: &[f64]) -> PairedInference {
    let maps = scores.len();
    let anytime = anytime_evidence(scores);
    if maps == 0 {
        return PairedInference {
            maps,
            score: 0.5,
            low: 0.0,
            high: 1.0,
            wilson_low: 0.0,
            wilson_high: 1.0,
            elo: 0.0,
            elo_low: elo_edge(0.0),
            elo_high: elo_edge(1.0),
            anytime,
            verdict: PromotionVerdict::Insufficient,
        };
    }

    let score = scores.iter().sum::<f64>() / maps as f64;
    let n = maps as f64;
    let z2 = Z_95 * Z_95;
    let denominator = 1.0 + z2 / n;
    let center = (score + z2 / (2.0 * n)) / denominator;
    let radius = Z_95 * ((score * (1.0 - score) / n + z2 / (4.0 * n * n)).sqrt()) / denominator;
    let wilson_low = (center - radius).clamp(0.0, 1.0);
    let wilson_high = (center + radius).clamp(0.0, 1.0);
    let (low, high) = betting_interval(scores);
    let challenger_evidence = anytime.challenger_p <= ANYTIME_TAIL_ALPHA;
    let incumbent_evidence = anytime.incumbent_p <= ANYTIME_TAIL_ALPHA;
    let verdict = if maps < PROMOTION_MIN_MAPS {
        PromotionVerdict::Insufficient
    } else if challenger_evidence && incumbent_evidence {
        // Strong evidence in both directions means the run is nonstationary
        // or its map order is pathological, not that either AI is promotable.
        PromotionVerdict::Inconclusive
    } else if challenger_evidence && low > 0.5 {
        PromotionVerdict::Promote
    } else if incumbent_evidence && high < 0.5 {
        PromotionVerdict::Retain
    } else {
        PromotionVerdict::Inconclusive
    };

    PairedInference {
        maps,
        score,
        low,
        high,
        wilson_low,
        wilson_high,
        elo: elo_edge(score),
        elo_low: elo_edge(low),
        elo_high: elo_edge(high),
        anytime,
        verdict,
    }
}

/// Trials per candidate edge when measuring what this run could have resolved.
/// Fixed so two readings of the same run agree. The measurement is a property
/// of the map count and break rate, not of when it was asked.
const RESOLUTION_SEED: u64 = 20_260_818;
const RESOLUTION_TRIALS: usize = 400;
/// The power a reported edge is quoted at.
const RESOLUTION_POWER: f64 = 0.80;
/// Bisection steps over the candidate edge. Twelve takes a [0.5, 1.0] bracket
/// below 0.0002 of paired-map score, far finer than the Elo it is printed as.
const RESOLUTION_STEPS: usize = 12;

/// Whether the gate would promote this score vector, without building the
/// interval.
///
/// `paired_inference` inverts the betting test over a grid of candidate means
/// to report an interval, which costs a thousand times what the verdict does.
/// The power search below asks only for the verdict, hundreds of thousands of
/// times, so it asks this instead. The two agree by construction — the
/// interval's `low > 0.5` is the same statement as the challenger direction
/// rejecting parity — and
/// `the_fast_verdict_agrees_with_the_full_one` holds them to it.
fn gate_would_promote(scores: &[f64]) -> bool {
    if scores.len() < PROMOTION_MIN_MAPS {
        return false;
    }
    let peaks = betting_peaks(scores, 0.5, PROMOTION_MIN_MAPS);
    let crossing_log_e = -(ANYTIME_TAIL_ALPHA.ln());
    let challenger = peaks.challenger_log_e >= crossing_log_e;
    let incumbent = peaks.incumbent_log_e >= crossing_log_e;
    challenger && !incumbent
}

/// ★★★★★ THE SMALLEST TRUE EDGE THIS RUN COULD HAVE RESOLVED.
///
/// `INCONCLUSIVE` is two completely different statements wearing one word:
/// "the arms are close" and "this run was too short to tell". Only the map
/// count and the break rate separate them, and a reader has neither to hand.
/// Left unsaid, the second reads as the first, and a real effect gets filed as
/// a null — which is exactly what this repository's log shows happening to
/// point estimates of +44 to +100 Elo-equivalent.
///
/// So the run says it. Simulate the *whole* gate — both conjuncts, the
/// `PROMOTION_MIN_MAPS` floor, the nonstationarity veto — on synthetic runs of
/// this length whose maps break at the rate this one did, and bisect for the
/// smallest edge it would have promoted `RESOLUTION_POWER` of the time.
///
/// ⚠ The break rate is taken from the observed run, which is the only estimate
/// available and is itself noisy on a short run. This is a scale, not a
/// certificate: it is meant to stop `INCONCLUSIVE` being read as `no effect`,
/// not to price the next experiment to the Elo.
fn resolvable_edge(maps: usize, break_rate: f64, seed: u64) -> Option<f64> {
    if maps < PROMOTION_MIN_MAPS || !(0.0..=1.0).contains(&break_rate) || break_rate <= 0.0 {
        return None;
    }
    let mut rng = Rng::new(seed);
    let power_at = |rng: &mut Rng, score: f64| -> f64 {
        // A map breaks with probability `break_rate`; when it does the
        // challenger takes it with whatever probability puts the mean at
        // `score`. An unbroken map contributes exactly 0.5, as it does live.
        let win_given_break = 0.5 + (score - 0.5) / break_rate;
        if !(0.0..=1.0).contains(&win_given_break) {
            return if score > 0.5 { 1.0 } else { 0.0 };
        }
        let mut promoted = 0usize;
        for _ in 0..RESOLUTION_TRIALS {
            let scores: Vec<f64> = (0..maps)
                .map(|_| {
                    if rng.f64() < break_rate {
                        f64::from(rng.f64() < win_given_break)
                    } else {
                        0.5
                    }
                })
                .collect();
            promoted += usize::from(gate_would_promote(&scores));
        }
        promoted as f64 / RESOLUTION_TRIALS as f64
    };
    // The gate cannot promote a score the break rate cannot reach.
    let mut low = 0.5;
    let mut high = 0.5 + 0.5 * break_rate;
    if power_at(&mut rng, high) < RESOLUTION_POWER {
        return None;
    }
    for _ in 0..RESOLUTION_STEPS {
        let middle = 0.5 * (low + high);
        if power_at(&mut rng, middle) >= RESOLUTION_POWER {
            high = middle;
        } else {
            low = middle;
        }
    }
    Some(elo_edge(high))
}

/// One line telling the reader which of the two `INCONCLUSIVE`s this is.
/// Enabled victory conditions this run produced but cannot resolve a change to.
///
/// Only the map-pairs holding a game the lane decided can turn on that lane, so
/// it can move the paired score by at most `decided / pairs`. When that ceiling
/// falls below the interval the run already reports, no treatment acting
/// through the lane can be seen here — however large its true effect is.
///
/// Returns `(lane, games it decided, its ceiling in points)`, most bounded
/// first. Lanes the profile never produced are left to the caller's separate
/// "never produced" line, which is a different and stronger statement.
fn unresolvable_lanes(
    enabled: &str,
    decided_by: &BTreeMap<String, usize>,
    won_by_entrants: &BTreeMap<String, usize>,
    pairs: usize,
    half_width_points: f64,
) -> Vec<(String, usize, f64)> {
    if pairs == 0 {
        return Vec::new();
    }
    let mut bounded: Vec<(String, usize, f64)> = enabled
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter_map(|name| {
            let decided = decided_by.get(name).copied().unwrap_or(0);
            if decided == 0 {
                return None;
            }
            // Only a game an entrant WON can move the paired score. On a field
            // profile a game the field takes is a draw for the pair, so a lane
            // the entrants never win has a ceiling of zero however often the
            // board produces it — which is the case this function existed to
            // catch and originally got wrong.
            let movable = won_by_entrants.get(name).copied().unwrap_or(0);
            let ceiling = 100.0 * movable as f64 / pairs as f64;
            (ceiling < half_width_points).then(|| (name.to_string(), decided, ceiling))
        })
        .collect();
    bounded.sort_by(|left, right| {
        left.2
            .partial_cmp(&right.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.0.cmp(&right.0))
    });
    bounded
}

fn resolution_note(maps: usize, resolved: usize, seed: u64) -> String {
    if maps == 0 {
        return String::new();
    }
    let break_rate = resolved as f64 / maps as f64;
    match resolvable_edge(maps, break_rate, seed) {
        Some(edge) => format!(
            "resolution: {maps} maps, {:.0}% of them breaking — this gate promotes a true edge of \
             about {edge:+.0} Elo-equivalent {:.0}% of the time, and anything smaller reads as \
             INCONCLUSIVE here whether or not it is real",
            100.0 * break_rate,
            100.0 * RESOLUTION_POWER,
        ),
        None => format!(
            "resolution: {maps} maps, {:.0}% of them breaking — too few for this gate to promote \
             any edge {:.0}% of the time; INCONCLUSIVE here carries no evidence either way",
            100.0 * break_rate,
            100.0 * RESOLUTION_POWER,
        ),
    }
}

fn pair_outcomes(scores: &[f64]) -> PairOutcomes {
    let mut outcomes = PairOutcomes::default();
    for score in scores {
        if (*score - 1.0).abs() < f64::EPSILON {
            outcomes.a_sweeps += 1;
        } else if score.abs() < f64::EPSILON {
            outcomes.b_sweeps += 1;
        } else if (*score - 0.5).abs() < f64::EPSILON {
            outcomes.neutral += 1;
        } else {
            outcomes.mixed_with_draw += 1;
        }
    }
    outcomes
}

/// Exact two-sided sign-test probability under equally likely directions.
/// Neutral mirrored maps are deliberately excluded: they contain effect-size
/// information but no evidence about which AI is directionally stronger.
fn exact_sign_p(a_favored: usize, b_favored: usize) -> f64 {
    let n = a_favored + b_favored;
    if n == 0 {
        return 1.0;
    }
    let tail = a_favored.min(b_favored);
    let n_f = n as f64;
    let mut log_choose = 0.0;
    let mut log_terms = Vec::with_capacity(tail + 1);
    for k in 0..=tail {
        log_terms.push(log_choose - n_f * std::f64::consts::LN_2);
        if k < tail {
            log_choose += ((n - k) as f64).ln() - ((k + 1) as f64).ln();
        }
    }
    let largest = log_terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lower_tail = largest.exp()
        * log_terms
            .iter()
            .map(|term| (*term - largest).exp())
            .sum::<f64>();
    (2.0 * lower_tail).min(1.0)
}

fn directional_outcomes(scores: &[f64]) -> DirectionalOutcomes {
    let mut outcomes = DirectionalOutcomes::default();
    for score in scores {
        if *score > 0.5 + f64::EPSILON {
            outcomes.challenger_favored += 1;
        } else if *score < 0.5 - f64::EPSILON {
            outcomes.incumbent_favored += 1;
        } else {
            outcomes.neutral += 1;
        }
    }
    outcomes
}

/// How much of the map set a direction statistic actually rests on.
///
/// A mirrored A/B between close agents splits most maps by construction, and
/// a split carries no direction. The win statistic is therefore computed
/// from only the maps that broke, while the terminal-score statistic — being
/// continuous — breaks on nearly all of them. Reporting the two resolutions
/// side by side is what stops a 5-0 win margin drawn from five maps being
/// read as stronger evidence than a 10-8 score margin drawn from eighteen.
fn resolved_maps(directions: &DirectionalOutcomes) -> usize {
    directions.challenger_favored + directions.incumbent_favored
}

/// Sign of a direction: `Some(true)` favours the challenger, `Some(false)`
/// the incumbent, `None` when the maps that broke split evenly.
fn direction_sign(directions: &DirectionalOutcomes) -> Option<bool> {
    match directions
        .challenger_favored
        .cmp(&directions.incumbent_favored)
    {
        std::cmp::Ordering::Greater => Some(true),
        std::cmp::Ordering::Less => Some(false),
        std::cmp::Ordering::Equal => None,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PlanTrace {
    observations: usize,
    switches: usize,
    rush_observations: usize,
    ever_rushed: bool,
    war_enabled: bool,
    war_active_observations: usize,
    targets: BTreeMap<String, usize>,
    last_target: Option<String>,
    strategy_switches: usize,
    strategy_turns: BTreeMap<String, usize>,
    midgame_observations: usize,
    midgame_strategy_switches: usize,
    midgame_boundary_switches: usize,
    midgame_unanchored_switches: usize,
    midgame_war_boundary_switches: usize,
    midgame_threat_boundary_switches: usize,
    midgame_city_deficit_boundary_switches: usize,
    midgame_strategy_turns: BTreeMap<String, usize>,
    midgame_transitions: BTreeMap<String, usize>,
    last_strategy: Option<String>,
    last_context: Option<StrategyContext>,
    /// Explored-plot count sampled the first time the seat is observed at or
    /// past each of `EXPLORATION_MARKS`; `None` when the game ended first.
    revealed_at_marks: [Option<usize>; EXPLORATION_MARKS.len()],
    /// Turn each rival (major or minor) was first observed in the seat's met
    /// set. Attribution lags an opponent-turn contact by at most one
    /// observation, which is the same for both arms.
    meet_turns: BTreeMap<usize, u32>,
    /// Turn the seat's first natural-wonder discovery was observed.
    first_wonder_turn: Option<u32>,
    /// Villages this seat had claimed the first time it was observed at or
    /// past the middle exploration mark; `None` when the game ended first.
    villages_by_mark: Option<i64>,
    /// Most reconnaissance-class units this seat ever fielded at once.
    recon_peak: usize,
    /// Villages standing on the whole board at first observation — the
    /// denominator that turns final claims into a contested share.
    board_villages: Option<usize>,
    /// Barbarian camps standing within six tiles of one of the seat's own
    /// cities the first time the seat is observed at or past the middle
    /// exploration mark; `None` when the game ended first. Six tiles is the
    /// engine's own near-a-city radius for the camp-destroyed moment.
    camps_near_home_by_mark: Option<usize>,
    /// Cities the seat holds the first time it is observed at or past t60 —
    /// the opening-tempo correlate the live ladder records as
    /// `cities_at_60`; `None` when the game ended first.
    cities_at_t60: Option<usize>,
}

/// Turns at which each seat's explored-plot count is sampled. These match the
/// marks `explore_commit_sweeps_more_ground` (src/ai.rs) prints, so harness
/// numbers and that census read on one scale.
const EXPLORATION_MARKS: [u32; 3] = [30, 50, 70];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StrategyContext {
    at_major_war: bool,
    threatened: bool,
    city_deficit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanObservation {
    target: &'static str,
    strategy: &'static str,
    rush: bool,
    war_enabled: bool,
    war_active: bool,
    context: StrategyContext,
    midgame: bool,
}

impl PlanTrace {
    fn observe(&mut self, observation: PlanObservation) {
        let PlanObservation {
            target,
            strategy,
            rush,
            war_enabled,
            war_active,
            context,
            midgame,
        } = observation;
        if self
            .last_target
            .as_deref()
            .is_some_and(|previous| previous != target)
        {
            self.switches += 1;
        }
        self.observations += 1;
        self.rush_observations += rush as usize;
        self.ever_rushed |= rush;
        self.war_enabled |= war_enabled;
        self.war_active_observations += war_active as usize;
        *self.targets.entry(target.to_string()).or_default() += 1;
        self.last_target = Some(target.to_string());

        let strategy_changed = self
            .last_strategy
            .as_deref()
            .is_some_and(|previous| previous != strategy);
        if strategy_changed {
            self.strategy_switches += 1;
            if midgame {
                self.midgame_strategy_switches += 1;
                let previous = self.last_strategy.as_deref().unwrap();
                *self
                    .midgame_transitions
                    .entry(format!("{previous}->{strategy}"))
                    .or_default() += 1;
                let previous_context = self.last_context.unwrap();
                let war_changed = previous_context.at_major_war != context.at_major_war;
                let threat_changed = previous_context.threatened != context.threatened;
                let city_deficit_changed = previous_context.city_deficit != context.city_deficit;
                self.midgame_war_boundary_switches += war_changed as usize;
                self.midgame_threat_boundary_switches += threat_changed as usize;
                self.midgame_city_deficit_boundary_switches += city_deficit_changed as usize;
                if war_changed || threat_changed || city_deficit_changed {
                    self.midgame_boundary_switches += 1;
                } else {
                    self.midgame_unanchored_switches += 1;
                }
            }
        }
        *self.strategy_turns.entry(strategy.to_string()).or_default() += 1;
        if midgame {
            self.midgame_observations += 1;
            *self
                .midgame_strategy_turns
                .entry(strategy.to_string())
                .or_default() += 1;
        }
        self.last_strategy = Some(strategy.to_string());
        self.last_context = Some(context);
    }

    /// Sample the exploration facts that cannot be read from the final
    /// position: when ground was revealed, when each contact was made, when
    /// villages were claimed, and how large the recon fleet ever grew.
    fn observe_exploration(&mut self, g: &Game, pid: usize) {
        // The whole board's village endowment, once, before anyone consumes
        // it: the denominator that turns final claims into a contested share.
        if self.board_villages.is_none() {
            self.board_villages = Some(
                g.map
                    .tiles
                    .values()
                    .filter(|tile| {
                        matches!(
                            tile.improvement.as_deref(),
                            Some("goody_hut" | "meteor_goody")
                        )
                    })
                    .count(),
            );
        }
        for (slot, mark) in EXPLORATION_MARKS.iter().enumerate() {
            if g.turn >= *mark && self.revealed_at_marks[slot].is_none() {
                self.revealed_at_marks[slot] = Some(g.players[pid].explored.len());
            }
        }
        // Villages are an expiring prize, so WHEN they were claimed matters
        // as much as how many: sample the engine counter at the middle mark.
        if g.turn >= EXPLORATION_MARKS[1] && self.villages_by_mark.is_none() {
            self.villages_by_mark = Some(
                g.players[pid]
                    .counters
                    .get("goody_huts_claimed")
                    .copied()
                    .unwrap_or(0),
            );
        }
        // Early-game barbarian exposure: how many camps stand beside this
        // seat's cities when the opening is over. Sampled, because by the
        // final position the world era has moved and camps have churned.
        if g.turn >= EXPLORATION_MARKS[1] && self.camps_near_home_by_mark.is_none() {
            let homes: Vec<_> = g
                .player_city_ids(pid)
                .iter()
                .map(|cid| g.cities[cid].pos)
                .collect();
            self.camps_near_home_by_mark = Some(
                g.barb_camps
                    .keys()
                    .filter(|camp| homes.iter().any(|home| g.wdist(**camp, *home) <= 6))
                    .count(),
            );
        }
        if g.turn >= 60 && self.cities_at_t60.is_none() {
            self.cities_at_t60 = Some(g.player_city_ids(pid).len());
        }
        let recon_now = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid && g.rules.units[unit.kind].promotion_class == "recon"
            })
            .count();
        self.recon_peak = self.recon_peak.max(recon_now);
        for other in &g.players[pid].met {
            self.meet_turns.entry(*other).or_insert(g.turn);
        }
        if self.first_wonder_turn.is_none() && !g.players[pid].discovered_natural_wonders.is_empty()
        {
            self.first_wonder_turn = Some(g.turn);
        }
    }

    /// Target used on the most observed player-turns. A tie keeps the final
    /// target, matching the tournament's dominant-strategy attribution.
    fn dominant_target(&self) -> &str {
        let most = self.targets.values().copied().max().unwrap_or(0);
        if self
            .last_target
            .as_ref()
            .is_some_and(|target| self.targets.get(target) == Some(&most))
        {
            return self.last_target.as_deref().unwrap();
        }
        self.targets
            .iter()
            .find(|(_, turns)| **turns == most)
            .map_or("unreported", |(target, _)| target.as_str())
    }
}

fn plan_observation(g: &Game, pid: usize, ai: &dyn Ai) -> PlanObservation {
    let midgame = g.turn >= g.standard_duration(60) && g.turn < g.standard_duration(180);
    let at_major_war = g.players.iter().any(|player| {
        player.id != pid
            && player.alive
            && !player.is_minor
            && !player.is_barbarian
            && g.is_at_war(pid, player.id)
    });
    ai.plan_report().map_or(
        PlanObservation {
            target: "unreported",
            strategy: "unreported",
            rush: false,
            war_enabled: false,
            war_active: false,
            context: StrategyContext {
                at_major_war,
                threatened: false,
                city_deficit: false,
            },
            midgame,
        },
        |plan| {
            let war_enabled = plan.war.as_ref().is_some_and(|war| war.enabled);
            let war_active = plan.war.as_ref().is_some_and(|war| war.active);
            PlanObservation {
                target: plan.victory_target.unwrap_or("adaptive"),
                strategy: plan.strategy,
                rush: plan.rush,
                war_enabled,
                war_active,
                context: StrategyContext {
                    at_major_war,
                    threatened: plan.threatened_city.is_some(),
                    city_deficit: g.player_city_ids(pid).len() < plan.desired_cities,
                },
                midgame,
            }
        },
    )
}

fn plan_target(g: &Game, pid: usize, ai: &dyn Ai) -> &'static str {
    plan_observation(g, pid, ai).target
}

fn run_traced_game(
    game: &mut Game,
    ais: &mut [Box<dyn Ai>],
    traced_players: usize,
) -> Vec<PlanTrace> {
    let mut traces: Vec<PlanTrace> = (0..traced_players).map(|_| PlanTrace::default()).collect();
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        ais[pid].take_turn(game, pid);
        if pid < traced_players {
            let observation = plan_observation(game, pid, ais[pid].as_ref());
            traces[pid].observe(observation);
            traces[pid].observe_exploration(game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    traces
}

#[derive(Default)]
struct TargetOutcome {
    games: usize,
    wins: usize,
}

#[derive(Default)]
struct Metrics {
    games: usize,
    wins: usize,
    score: f64,
    cities: f64,
    population: f64,
    techs: f64,
    civics: f64,
    districts: f64,
    buildings: f64,
    military: f64,
    gold: f64,
    faith: f64,
    tourists: f64,
    dvp: f64,
    envoys: f64,
    suzerainties: f64,
    military_units: f64,
    civilian_units: f64,
    religious_units: f64,
    food_yield: f64,
    production_yield: f64,
    science_yield: f64,
    culture_yield: f64,
    queued_cost: f64,
    settlers: f64,
    builders: f64,
    traders: f64,
    active_routes: f64,
    trade_capacity: f64,
    support_units: f64,
    missionaries: f64,
    victories: BTreeMap<String, usize>,
    final_targets: BTreeMap<String, usize>,
    dominant_targets: BTreeMap<String, usize>,
    target_outcomes: BTreeMap<String, TargetOutcome>,
    plan_turns: BTreeMap<String, usize>,
    plan_observations: usize,
    plan_switches: usize,
    strategy_turns: BTreeMap<String, usize>,
    strategy_switches: usize,
    midgame_strategy_turns: BTreeMap<String, usize>,
    midgame_observations: usize,
    midgame_strategy_switches: usize,
    midgame_boundary_switches: usize,
    midgame_unanchored_switches: usize,
    midgame_war_boundary_switches: usize,
    midgame_threat_boundary_switches: usize,
    midgame_city_deficit_boundary_switches: usize,
    midgame_transitions: BTreeMap<String, usize>,
    rush_seats: usize,
    rush_turns: usize,
    /// Reviews summed over every seat this entrant played, so a run can say
    /// whether the macro search ever ran. Stays zero for agents that do not
    /// search, which is honest rather than missing.
    census: ReviewCensus,
    /// Seats whose agent reports a search at all, distinguishing "searched
    /// zero times" from "has no search to report".
    searching_seats: usize,
    /// Actual production telemetry for the adaptive-expansion evaluator arms.
    /// Kept separately from city totals because a queue decision is not a
    /// founded city, and neither is a plan predicate.
    expansion_census: ExpansionCensus,
    expansion_reporting_seats: usize,
    dispatch_action_seats: usize,
    dispatch_settler_seats: usize,
    dispatch_late_settler_seats: usize,
    advanced_late_settler_seats: usize,
    expansion_deadlines: Option<(u32, u32)>,
    war_reporting_seats: usize,
    war_plan_seats: usize,
    war_active_turns: usize,
    war_appointments: u32,
    war_breakthroughs: u32,
    war_mobilizations: u32,
    war_declarations: u32,
    war_complete_declarations: u32,
    war_objectives_captured: u32,
    war_objectives_captured_within_ten: u32,
    war_appointment_to_tech: Vec<u32>,
    war_tech_to_declaration: Vec<u32>,
    war_declaration_to_capture: Vec<u32>,
    war_aborts: BTreeMap<&'static str, u32>,
    /// Exploration telemetry. Sums over seats; the paired seat counts let a
    /// mean skip games that ended before a mark or never made the discovery.
    revealed_at_marks: [f64; EXPLORATION_MARKS.len()],
    revealed_mark_seats: [usize; EXPLORATION_MARKS.len()],
    goody_huts_claimed: f64,
    meteor_goodies_claimed: f64,
    era_score: f64,
    natural_wonders_discovered: f64,
    first_wonder_turn_sum: f64,
    first_wonder_seats: usize,
    minors_met: f64,
    minors_met_by_t50: f64,
    first_minor_meet_turn_sum: f64,
    first_minor_meet_seats: usize,
    villages_by_mark: f64,
    villages_by_mark_seats: usize,
    recon_peak: f64,
    board_villages: f64,
    board_village_seats: usize,
    majors_met: f64,
    majors_met_by_t50: f64,
    first_major_meet_turn_sum: f64,
    first_major_meet_seats: usize,
    /// Barbarian ledger, both sides: what the seat took from the barbarians
    /// and what the barbarians took from it, plus the camp exposure that
    /// frames those numbers.
    camps_cleared: f64,
    barbs_killed: f64,
    lost_to_barbarians: f64,
    civilians_lost_to_barbarians: f64,
    camps_near_home: f64,
    camps_near_home_seats: usize,
    camps_standing: f64,
    cities_at_t60: f64,
    cities_at_t60_seats: usize,
}

impl Metrics {
    fn record_war(&mut self, trace: &PlanTrace, report: Option<&WarPlanReport>) {
        if !trace.war_enabled {
            return;
        }
        self.war_reporting_seats += 1;
        self.war_active_turns += trace.war_active_observations;
        let Some(report) = report else {
            return;
        };
        self.war_plan_seats += usize::from(report.appointments > 0);
        self.war_appointments += report.appointments;
        self.war_breakthroughs += report.breakthroughs;
        self.war_mobilizations += report.mobilizations;
        self.war_declarations += report.declarations;
        self.war_complete_declarations += report.complete_package_declarations;
        self.war_objectives_captured += report.objectives_captured;
        self.war_objectives_captured_within_ten += report.objectives_captured_within_ten;
        self.war_appointment_to_tech
            .extend(report.appointment_to_tech_samples.iter().copied());
        self.war_tech_to_declaration
            .extend(report.tech_to_declaration_samples.iter().copied());
        self.war_declaration_to_capture
            .extend(report.declaration_to_capture_samples.iter().copied());
        for (reason, count) in &report.aborts {
            *self.war_aborts.entry(*reason).or_default() += count;
        }
    }

    fn record(&mut self, g: &Game, pid: usize, won: bool, final_target: &str, trace: &PlanTrace) {
        let cities = g.player_city_ids(pid);
        self.games += 1;
        self.wins += won as usize;
        *self
            .final_targets
            .entry(final_target.to_string())
            .or_default() += 1;
        let dominant_target = trace.dominant_target().to_string();
        *self
            .dominant_targets
            .entry(dominant_target.clone())
            .or_default() += 1;
        let outcome = self.target_outcomes.entry(dominant_target).or_default();
        outcome.games += 1;
        outcome.wins += won as usize;
        self.plan_observations += trace.observations;
        self.plan_switches += trace.switches;
        self.strategy_switches += trace.strategy_switches;
        self.midgame_observations += trace.midgame_observations;
        self.midgame_strategy_switches += trace.midgame_strategy_switches;
        self.midgame_boundary_switches += trace.midgame_boundary_switches;
        self.midgame_unanchored_switches += trace.midgame_unanchored_switches;
        self.midgame_war_boundary_switches += trace.midgame_war_boundary_switches;
        self.midgame_threat_boundary_switches += trace.midgame_threat_boundary_switches;
        self.midgame_city_deficit_boundary_switches += trace.midgame_city_deficit_boundary_switches;
        self.rush_seats += trace.ever_rushed as usize;
        self.rush_turns += trace.rush_observations;
        for (target, turns) in &trace.targets {
            *self.plan_turns.entry(target.clone()).or_default() += turns;
        }
        for (strategy, turns) in &trace.strategy_turns {
            *self.strategy_turns.entry(strategy.clone()).or_default() += turns;
        }
        for (strategy, turns) in &trace.midgame_strategy_turns {
            *self
                .midgame_strategy_turns
                .entry(strategy.clone())
                .or_default() += turns;
        }
        for (transition, count) in &trace.midgame_transitions {
            *self
                .midgame_transitions
                .entry(transition.clone())
                .or_default() += count;
        }
        if won {
            *self
                .victories
                .entry(
                    g.victory_type
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default() += 1;
        }
        self.score += g.score(pid) as f64;
        self.cities += cities.len() as f64;
        self.population += cities.iter().map(|cid| g.cities[cid].pop).sum::<i32>() as f64;
        self.techs += g.players[pid].techs.len() as f64;
        self.civics += g.players[pid].civics.len() as f64;
        self.districts += cities
            .iter()
            .map(|cid| g.cities[cid].districts.len())
            .sum::<usize>() as f64;
        self.buildings += cities
            .iter()
            .map(|cid| g.cities[cid].buildings.len())
            .sum::<usize>() as f64;
        self.military += g.military_power(pid);
        self.gold += g.players[pid].gold;
        self.faith += g.players[pid].faith;
        self.tourists += g.foreign_tourists(pid) as f64;
        self.dvp += g.players[pid].dvp as f64;
        self.envoys += g.players[pid]
            .envoys
            .iter()
            .map(|(_, count)| *count)
            .sum::<i64>() as f64;
        self.suzerainties += g
            .players
            .iter()
            .filter(|minor| minor.alive && minor.is_minor && g.suzerain_of(minor.id) == Some(pid))
            .count() as f64;
        self.active_routes += g.active_routes(pid) as f64;
        self.trade_capacity += g.trade_capacity(pid) as f64;
        for unit in g.units.values().filter(|u| u.owner == pid) {
            match unit.kind.as_str() {
                "settler" => self.settlers += 1.0,
                "builder" => self.builders += 1.0,
                "trader" => self.traders += 1.0,
                "missionary" => self.missionaries += 1.0,
                _ if g.rules.units[unit.kind].class == "support" => {
                    self.support_units += 1.0
                }
                _ => {}
            }
            if g.rules.units[unit.kind].class == "military" {
                self.military_units += 1.0;
            } else {
                self.civilian_units += 1.0;
            }
            if g.rules.units[unit.kind].class == "religious" {
                self.religious_units += 1.0;
            }
        }
        for cid in &cities {
            let yields = g.city_yields(*cid);
            self.food_yield += yields.food;
            self.production_yield += yields.production;
            self.science_yield += yields.science;
            self.culture_yield += yields.culture;
            if let Some(item) = g.cities[cid].queue.first() {
                self.queued_cost += g.item_cost_for(pid, item);
            }
        }
        for (slot, revealed) in trace.revealed_at_marks.iter().enumerate() {
            if let Some(revealed) = revealed {
                self.revealed_at_marks[slot] += *revealed as f64;
                self.revealed_mark_seats[slot] += 1;
            }
        }
        let counter = |name: &str| g.players[pid].counters.get(name).copied().unwrap_or(0) as f64;
        self.goody_huts_claimed += counter("goody_huts_claimed");
        self.meteor_goodies_claimed += counter("meteor_goodies_claimed");
        self.camps_cleared += counter("camps");
        self.barbs_killed += counter("barbs_killed");
        self.lost_to_barbarians += counter("lost_to_barbarians");
        self.civilians_lost_to_barbarians += counter("civilians_lost_to_barbarians");
        if let Some(camps) = trace.camps_near_home_by_mark {
            self.camps_near_home += camps as f64;
            self.camps_near_home_seats += 1;
        }
        self.camps_standing += g.barb_camps.len() as f64;
        if let Some(cities) = trace.cities_at_t60 {
            self.cities_at_t60 += cities as f64;
            self.cities_at_t60_seats += 1;
        }
        self.era_score += g.players[pid].era_score as f64;
        self.natural_wonders_discovered += g.players[pid].discovered_natural_wonders.len() as f64;
        if let Some(early) = trace.villages_by_mark {
            self.villages_by_mark += early as f64;
            self.villages_by_mark_seats += 1;
        }
        self.recon_peak += trace.recon_peak as f64;
        if let Some(board) = trace.board_villages {
            self.board_villages += board as f64;
            self.board_village_seats += 1;
        }
        if let Some(turn) = trace.first_wonder_turn {
            self.first_wonder_turn_sum += turn as f64;
            self.first_wonder_seats += 1;
        }
        let mut first_minor: Option<u32> = None;
        let mut first_major: Option<u32> = None;
        for (other, turn) in &trace.meet_turns {
            let Some(other_player) = g.players.get(*other) else {
                continue;
            };
            if *other == pid || other_player.is_barbarian || other_player.is_free_city {
                continue;
            }
            if other_player.is_minor {
                self.minors_met += 1.0;
                self.minors_met_by_t50 += f64::from(u8::from(*turn <= 50));
                first_minor = Some(first_minor.map_or(*turn, |seen| seen.min(*turn)));
            } else {
                self.majors_met += 1.0;
                self.majors_met_by_t50 += f64::from(u8::from(*turn <= 50));
                first_major = Some(first_major.map_or(*turn, |seen| seen.min(*turn)));
            }
        }
        if let Some(turn) = first_minor {
            self.first_minor_meet_turn_sum += turn as f64;
            self.first_minor_meet_seats += 1;
        }
        if let Some(turn) = first_major {
            self.first_major_meet_turn_sum += turn as f64;
            self.first_major_meet_seats += 1;
        }
    }
}

/// Mean turn over the seats that made the observation at all, or "-" when
/// none did — a seat that never met a rival has no meet turn to average.
fn mean_turn(sum: f64, seats: usize) -> String {
    if seats == 0 {
        "-".to_string()
    } else {
        format!("t{:.0}", sum / seats as f64)
    }
}

fn target_shares(metrics: &Metrics) -> String {
    metrics
        .plan_turns
        .iter()
        .map(|(target, turns)| {
            let share = 100.0 * *turns as f64 / metrics.plan_observations.max(1) as f64;
            format!("{target} {share:.1}%")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn shares(turns: &BTreeMap<String, usize>, observations: usize) -> String {
    turns
        .iter()
        .map(|(label, turns)| {
            let share = 100.0 * *turns as f64 / observations.max(1) as f64;
            format!("{label} {share:.1}%")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn transition_counts(transitions: &BTreeMap<String, usize>) -> String {
    let mut ranked: Vec<(&str, usize)> = transitions
        .iter()
        .map(|(transition, count)| (transition.as_str(), *count))
        .collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    ranked
        .into_iter()
        .map(|(transition, count)| format!("{transition} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Keep production telemetry auditable without making evaluator output depend
/// on worker completion order.
fn turn_list(turns: &[u32]) -> String {
    let mut sorted = turns.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        "none".to_string()
    } else {
        sorted
            .into_iter()
            .map(|turn| turn.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn median_turns(turns: &[u32]) -> String {
    let mut sorted = turns.to_vec();
    sorted.sort_unstable();
    match sorted.len() {
        0 => "unresolved".to_string(),
        len if len % 2 == 1 => format!("{:.1}", sorted[len / 2] as f64),
        len => format!(
            "{:.1}",
            (sorted[len / 2 - 1] as f64 + sorted[len / 2] as f64) / 2.0
        ),
    }
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// What the number under the gate is actually worth, said out loud.
///
/// The gate decides; this describes. Separating them is the whole of R3: a
/// promotion gate accepts when the observed effect is large enough, so
/// conditioning on "passed" conditions on the estimate being large, and every
/// headline size in this repository is inflated by an amount that grows as the
/// true effect shrinks. `+207` against a re-measured `+86` is the signature.
///
/// ⚠ Every branch must contain a token that `unevidenced_effect_sizes` in
/// `tools/civvis_collab.py` accepts as provenance — each one names a seed. If
/// it did not, this tool would print numbers the repository's own docs gate
/// then refuses, and the two halves of R3 would disagree.
fn effect_size_line(
    elo: f64,
    verdict: PromotionVerdict,
    seed: u64,
    confirm: Option<u64>,
) -> String {
    let selected_on_size = matches!(
        verdict,
        PromotionVerdict::Promote | PromotionVerdict::Retain
    );
    match (confirm, selected_on_size) {
        (Some(prior), _) => format!(
            "effect size:    {elo:+.0} (CONFIRMED — measured on seed {seed}, disjoint from the \
             discovery seed {prior}; quotable, and quote this estimate rather than the \
             discovery one)"
        ),
        (None, true) => format!(
            "effect size:    {elo:+.0} (DISCOVERY ESTIMATE — selected on passing the gate, so \
             biased upward; not quotable until confirmed on a disjoint seed: rerun with \
             --seed <new> --confirm {seed})"
        ),
        (None, false) => format!(
            "effect size:    {elo:+.0} (not gate-selected — the gate did not fire, so this \
             estimate is not conditioned on being large; still a single run on seed {seed})"
        ),
    }
}

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn native_competitions_requested(args: &[String]) -> bool {
    args.iter()
        .any(|argument| argument == "--native-competitions")
}

/// Apply the world rules that the evaluator is explicitly pricing.
///
/// Native competitions remain opt-in so historical evaluator records keep
/// their ruleset. A run that asks for them must set the flag on every seat of
/// every paired game, rather than merely report the requested command line.
fn configure_evaluation_game(
    game: &mut Game,
    victory_conditions: VictoryConditions,
    native_competitions: bool,
) {
    game.victory_conditions = victory_conditions;
    game.native_competitions = native_competitions;
}

#[derive(Clone, Copy)]
struct MatrixProfile {
    name: &'static str,
    players: usize,
    width: i32,
    height: i32,
    city_states: usize,
    turns: u32,
    speed: &'static str,
    /// The victory checkboxes this profile's games leave enabled. Per-profile
    /// because the two requirements buy different things with the list: the
    /// Strength question is "should this displace the incumbent *in
    /// deployment*", so its games must be able to end every way a deployment
    /// game can; the NoRegression tripwire keeps the three-victory set for the
    /// ~23% higher decisive-map rate measured in `docs/EVAL.md` (2026-08-11,
    /// the gate's three-victory entry).
    victories: &'static str,
    /// The agents seated in the chairs the two entrants do not take, as
    /// `--field` takes them; empty for a profile where every chair is an
    /// entrant.
    ///
    /// ★★★★★ THIS IS WHY THE MATRIX WAS BLIND TO TWO THIRDS OF WHAT KILLS US.
    /// Both fieldless profiles seat `AdvancedAi` variants in every chair,
    /// `AdvancedAi` routes to religion, and the deployment profile was measured
    /// on 2026-08-18 producing **religious and score and zero diplomatic, zero
    /// culture over 40 games** — twice, on two disjoint seed streams. The live
    /// Civilization VI ladder over the same period lost **41 games to a rival's
    /// diplomatic victory and 24 to culture**: 65 of 310 attempts, ended by two
    /// conditions no promotion decision in this repository's history could see.
    field: &'static str,
    /// The three world axes every recorded profile shares, held here rather
    /// than hard-coded inside `matrix_child_args` so that a profile *names its
    /// whole world*. They were constants in the child builder until
    /// `--profile` made the same profile reachable from a plain run: two
    /// expansions of one name that agreed only by coincidence would be the
    /// exact defect this file already refuses elsewhere — reporting a profile
    /// the run does not have.
    map: &'static str,
    shape: &'static str,
    poles: &'static str,
    randomize_civs: bool,
    /// Relative cost of one game on this profile, used only to split the
    /// concurrency budget. The deployment shape measures about twice the
    /// compact one.
    cost_weight: usize,
    requirement: MatrixRequirement,
}

struct MatrixChildRequest<'a> {
    challenger: &'a str,
    incumbent: &'a str,
    pairs: usize,
    jobs: usize,
    seed: u64,
    profile: MatrixProfile,
    difficulty: &'a str,
    require_artifacts: bool,
    confirm_seed: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MatrixRequirement {
    NoRegression,
    Strength,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MatrixVerdict {
    Pass,
    Retain,
    Inconclusive,
    Insufficient,
}

const PROMOTION_PROFILES: [MatrixProfile; 3] = [
    MatrixProfile {
        name: "compact-standard",
        players: 4,
        width: 24,
        height: 16,
        city_states: 4,
        turns: 500,
        speed: "standard",
        victories: "science,culture,domination",
        field: "",
        map: "continents",
        shape: "planet",
        poles: "poles",
        randomize_civs: true,
        cost_weight: 1,
        requirement: MatrixRequirement::NoRegression,
    },
    MatrixProfile {
        name: "deployment-online",
        players: 6,
        width: 74,
        height: 46,
        city_states: 9,
        turns: 250,
        speed: "online",
        // The deployment's full set (#658 hard-coded three for both profiles;
        // changed 2026-08-14, see `docs/EVAL.md` — the Strength verdict now
        // attaches to the game the exhibition and the live bridge actually
        // play).
        victories: "science,culture,religious,diplomatic,domination,score",
        field: "",
        map: "continents",
        shape: "planet",
        poles: "poles",
        randomize_civs: true,
        cost_weight: 2,
        requirement: MatrixRequirement::Strength,
    },
    // ★★★★★ THE BOARD THE FRONT LINE ACTUALLY PLAYS ON.
    //
    // Same shape as `deployment-online`, and the only difference is who else is
    // in the game: the chairs the entrants do not take are seated with agents
    // that pursue the two lanes Firaxis' AI pursues. Fieldless that shape
    // produces diplomatic 0 and culture 0 of 40; with this field it produces
    // culture 11 and diplomatic 6 of 40 (seed 32000000, `docs/eval/`).
    //
    // ⚠ **`NoRegression`, deliberately, and this is the whole of the policy
    // choice.** A third `Strength` bar would make every future treatment clear
    // a profile it was never designed for, and this repository has no evidence
    // that would be a good trade. A tripwire adds what is missing without
    // adding a hurdle: a treatment that measurably *harms* the contested board
    // is refused, and one that is merely inconclusive there passes as before.
    // Raising it to `Strength` is a separate decision needing its own evidence.
    //
    // ⚠ Its numbers are not comparable to `deployment-online`'s. Two entrants
    // hold two chairs here instead of six, so a game is one contest rather than
    // three and a fixed pair count carries less information. Read the two
    // profiles as different questions, never as a replication.
    MatrixProfile {
        name: "deployment-contested",
        players: 6,
        width: 74,
        height: 46,
        city_states: 9,
        turns: 250,
        speed: "online",
        victories: "science,culture,religious,diplomatic,domination,score",
        field: "live_target_diplomatic,live_target_culture",
        map: "continents",
        shape: "planet",
        poles: "poles",
        randomize_civs: true,
        cost_weight: 2,
        requirement: MatrixRequirement::NoRegression,
    },
];

/// Stable separation between profile seed streams.
///
/// This must not depend on `--pairs`: increasing a preregistered sample must
/// extend each profile's existing seed prefix, not silently select a new one.
const MATRIX_PROFILE_SEED_STRIDE: u64 = 1_000_000;

/// World axes fixed by the recorded promotion profiles.
///
/// The matrix deliberately has no pass-through for these: accepting an axis
/// it does not supply to its children would silently report a different world
/// than the one the promotion gate names.
const MATRIX_PROFILE_FLAGS: [&str; 13] = [
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--turns",
    "--speed",
    "--map",
    "--shape",
    "--poles",
    "--victories",
    "--randomize-civs",
    // Native scored competitions add a global Diplomatic Victory ruleset.
    "--native-competitions",
    // Who else is on the board decides which victory conditions are reachable.
    "--field",
];

fn matrix_profile_flag(args: &[String]) -> Option<&'static str> {
    MATRIX_PROFILE_FLAGS
        .iter()
        .copied()
        .find(|flag| args.iter().any(|argument| argument == flag))
}

fn matrix_profile_seed(seed: u64, profile_index: usize) -> u64 {
    seed + profile_index as u64 * MATRIX_PROFILE_SEED_STRIDE
}

/// The world a named profile stands for, as the command line that produces it.
///
/// One expansion, two callers: the promotion matrix builds its children with
/// it, and `--profile <name>` gives a plain run the identical board. That
/// sharing is the point. Every recorded contested round in `docs/eval/` was
/// launched by hand-typing eleven axis flags, and a hand-typed board agrees
/// with the gate's board only for as long as nobody edits either — which is
/// how this repository has already published a profile line describing a
/// world the run did not have (`--artifact-dir`, refused a few hundred lines
/// below for exactly that reason).
fn profile_axis_args(profile: MatrixProfile) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--players".to_string(),
        profile.players.to_string(),
        "--width".to_string(),
        profile.width.to_string(),
        "--height".to_string(),
        profile.height.to_string(),
        "--city-states".to_string(),
        profile.city_states.to_string(),
        "--turns".to_string(),
        profile.turns.to_string(),
        "--speed".to_string(),
        profile.speed.to_string(),
        "--map".to_string(),
        profile.map.to_string(),
        "--shape".to_string(),
        profile.shape.to_string(),
        "--poles".to_string(),
        profile.poles.to_string(),
        "--victories".to_string(),
        profile.victories.to_string(),
    ];
    if profile.randomize_civs {
        args.push("--randomize-civs".to_string());
    }
    if !profile.field.is_empty() {
        args.push("--field".to_string());
        args.push(profile.field.to_string());
    }
    args
}

/// Resolve a `--profile <name>` request against the recorded profiles.
fn named_profile(name: &str) -> Option<MatrixProfile> {
    PROMOTION_PROFILES
        .into_iter()
        .find(|profile| profile.name == name)
}

fn profile_names() -> String {
    PROMOTION_PROFILES
        .iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Expand `--profile <name>` into the world axes that name stands for.
///
/// ★★★★★ WHY A NAME AND NOT ELEVEN FLAGS.
///
/// `deployment-contested` is the only board this repository has that produces
/// the conditions the live ladder actually loses to — 83 of 232 terminal games
/// taken by a rival's victory, 47 of them diplomatic and 27 culture
/// (`docs/EVAL_STATUS.md`). Until now it existed only *inside* `--matrix`, so
/// every single-arm round measured on it — four of them in `docs/eval/` —
/// retyped its axes by hand. Nothing checked those eleven flags against the
/// gate's own profile, and a round whose board has silently drifted from the
/// gate's board is not evidence about the gate.
///
/// Conflicting explicit axes are **refused, not overridden**, on the same
/// reasoning `MATRIX_PROFILE_FLAGS` already carries for the matrix: accepting
/// an axis the profile also supplies would report one world and play another.
/// `--difficulty`, `--pairs`, `--jobs`, `--seed` and `--confirm` are not world
/// axes and stay free.
fn expand_named_profile(args: Vec<String>) -> Result<Vec<String>, String> {
    let Some(index) = args.iter().position(|argument| argument == "--profile") else {
        return Ok(args);
    };
    let requested = args
        .get(index + 1)
        .cloned()
        .ok_or_else(|| format!("--profile needs a name; choose from {}", profile_names()))?;
    let Some(profile) = named_profile(&requested) else {
        return Err(format!(
            "--profile: unknown profile {requested:?}; choose from {}",
            profile_names()
        ));
    };
    if args.iter().any(|argument| argument == "--matrix") {
        return Err(
            "--profile names one board and --matrix runs all of them; choose one".to_string(),
        );
    }
    if let Some(flag) = matrix_profile_flag(&args) {
        return Err(format!(
            "--profile {requested} already fixes {flag}; passing it as well would report \
             {requested}'s world and play a different one. Drop {flag}, or drop --profile \
             and name every axis yourself"
        ));
    }
    // `--profile <name>` stays on the command line the run reports, because
    // the name is the provenance: a round that cites `deployment-contested`
    // has to be able to point at the run saying it played it.
    let mut expanded = args;
    expanded.extend(profile_axis_args(profile));
    Ok(expanded)
}

fn matrix_child_args(request: MatrixChildRequest<'_>) -> Vec<String> {
    let MatrixChildRequest {
        challenger,
        incumbent,
        pairs,
        jobs,
        seed,
        profile,
        difficulty,
        require_artifacts,
        confirm_seed,
    } = request;
    let mut args: Vec<String> = vec![
        challenger.to_string(),
        incumbent.to_string(),
        "--pairs".to_string(),
        pairs.to_string(),
        "--jobs".to_string(),
        jobs.max(1).to_string(),
        "--seed".to_string(),
        seed.to_string(),
    ];
    // The world itself, from the one expansion `--profile` also uses. The
    // command line may not pass `--field` alongside `--matrix` — the matrix
    // owns its profiles — but the matrix supplies it to the child that
    // declares one. The child is a plain `ai_eval` invocation with no
    // `--matrix`, so nothing here is bypassing that refusal.
    args.extend(profile_axis_args(profile));
    args.extend([
        "--difficulty".to_string(),
        difficulty.to_string(),
        // A profile matrix asks the explicitly replacement-oriented question
        // of whether the challenger should displace the incumbent. It is not
        // an attribution claim, so its child runs opt into multi-axis arms.
        "--deployment-comparison".to_string(),
    ]);
    if require_artifacts {
        args.push("--require-artifacts".to_string());
    }
    if let Some(confirm_seed) = confirm_seed {
        // A matrix has two independent seed streams. Preserve that shape when
        // the entire matrix is a confirmation: each child must name the prior
        // seed for its own profile, not the compact profile base seed.
        args.push("--confirm".to_string());
        args.push(confirm_seed.to_string());
    }
    args
}

/// Check the inclusive seed prefixes used by a discovery run and its
/// confirmation. Different bases are not enough: `1000..=1049` and
/// `1025..=1074` are still the same maps for half of the run. Keep this
/// arithmetic in the evaluator so a confirmation cannot accidentally reuse a
/// selected discovery map and call the result independent evidence.
fn disjoint_seed_prefixes(seed: u64, prior: u64, pairs: usize) -> Result<(), String> {
    let width = u64::try_from(pairs.saturating_sub(1))
        .map_err(|_| "--pairs is too large to form a seed prefix".to_string())?;
    let end = |start: u64| {
        start.checked_add(width).ok_or_else(|| {
            format!("seed prefix starting at {start} overflows u64; choose a lower --seed")
        })
    };
    let discovery_end = end(seed)?;
    let confirmation_end = end(prior)?;
    if seed <= confirmation_end && prior <= discovery_end {
        return Err(format!(
            "discovery seed prefix {seed}..={discovery_end} overlaps confirmation prefix {prior}..={confirmation_end}; choose disjoint seed prefixes"
        ));
    }
    Ok(())
}

fn confirmation_base_seed(args: &[String], seed: u64, pairs: usize) -> Result<Option<u64>, String> {
    let Some(index) = args.iter().position(|arg| arg == "--confirm") else {
        return Ok(None);
    };
    let prior = args
        .get(index + 1)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "--confirm needs the base seed of the matrix being confirmed".to_string())?;
    disjoint_seed_prefixes(seed, prior, pairs)?;
    Ok(Some(prior))
}

fn matrix_confirmation_base_seed(
    args: &[String],
    seed: u64,
    pairs: usize,
) -> Result<Option<u64>, String> {
    confirmation_base_seed(args, seed, pairs)
}

fn matrix_verdict(output: &[u8]) -> Option<MatrixVerdict> {
    let output = String::from_utf8_lossy(output);
    if output.contains("promotion gate: PASS —") {
        Some(MatrixVerdict::Pass)
    } else if output.contains("promotion gate: RETAIN ") {
        Some(MatrixVerdict::Retain)
    } else if output.contains("promotion gate: INCONCLUSIVE —") {
        Some(MatrixVerdict::Inconclusive)
    } else if output.contains("promotion gate: INSUFFICIENT —") {
        Some(MatrixVerdict::Insufficient)
    } else {
        None
    }
}

fn matrix_profile_accepts(requirement: MatrixRequirement, verdict: MatrixVerdict) -> bool {
    match requirement {
        MatrixRequirement::Strength => verdict == MatrixVerdict::Pass,
        MatrixRequirement::NoRegression => {
            matches!(verdict, MatrixVerdict::Pass | MatrixVerdict::Inconclusive)
        }
    }
}

/// Split the concurrent matrix budget around the measured critical path.
///
/// The 6p 74x46 deployment profile has roughly twice the work of the compact
/// 4p 24x16 profile in repeated matrix runs. An equal split therefore leaves
/// compact idle while deployment determines wall time; integer division also
/// discarded every odd remainder. Keep every requested worker and give the
/// heavier profile about two thirds, while retaining at least one per profile.
fn matrix_job_budgets(total_jobs: usize) -> Vec<usize> {
    let weights: Vec<usize> = PROMOTION_PROFILES
        .iter()
        .map(|profile| profile.cost_weight.max(1))
        .collect();
    if total_jobs < weights.len() {
        // These run sequentially, so each child can use the sole worker.
        return vec![1; weights.len()];
    }
    let total_weight: usize = weights.iter().sum();
    let mut budgets: Vec<usize> = weights
        .iter()
        .map(|weight| (total_jobs * weight / total_weight).max(1))
        .collect();
    // Keep every requested worker, and never hand one to a profile that has
    // none: the integer division discards remainders and the floor above can
    // overshoot when the budget is barely larger than the profile count.
    let heaviest_first = |budgets: &[usize]| -> Vec<usize> {
        let mut order: Vec<usize> = (0..weights.len()).collect();
        order.sort_by_key(|index| (std::cmp::Reverse(weights[*index]), budgets[*index], *index));
        order
    };
    while budgets.iter().sum::<usize>() < total_jobs {
        let index = heaviest_first(&budgets)[0];
        budgets[index] += 1;
    }
    while budgets.iter().sum::<usize>() > total_jobs {
        let Some(index) = heaviest_first(&budgets)
            .into_iter()
            .rev()
            .find(|index| budgets[*index] > 1)
        else {
            break;
        };
        budgets[index] -= 1;
    }
    budgets
}

fn run_profile_matrix(args: &[String], challenger: &str, incumbent: &str) -> ! {
    if let Some(flag) = matrix_profile_flag(args) {
        eprintln!("--matrix owns the promotion profiles; remove conflicting profile flag {flag}");
        std::process::exit(2);
    }
    if args.iter().any(|argument| argument == "--allow-degraded") {
        eprintln!("--matrix never permits degraded agents; remove --allow-degraded");
        std::process::exit(2);
    }
    let pairs = number(args, "--pairs", 50).max(1) as usize;
    if pairs as u64 >= MATRIX_PROFILE_SEED_STRIDE {
        eprintln!(
            "--matrix supports fewer than {MATRIX_PROFILE_SEED_STRIDE} pairs so profile seed streams remain disjoint"
        );
        std::process::exit(2);
    }
    let artifact_dir = text(args, "--artifact-dir", ARTIFACT_DIR);
    if artifact_dir != ARTIFACT_DIR {
        eprintln!(
            "--matrix cannot use --artifact-dir {artifact_dir}; agent construction resolves {ARTIFACT_DIR}"
        );
        std::process::exit(2);
    }
    let total_jobs = match number(args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let job_budgets = matrix_job_budgets(total_jobs);
    let seed = number(args, "--seed", 4000).max(0) as u64;
    let confirm_base_seed = match matrix_confirmation_base_seed(args, seed, pairs) {
        Ok(seed) => seed,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let difficulty = text(args, "--difficulty", &default_difficulty());
    let require_artifacts = args
        .iter()
        .any(|argument| argument == "--require-artifacts");
    // Each profile child stops on its own decisive verdict; the parent still
    // reads every child's gate line exactly as before.
    let stop_when_decisive = args.iter().any(|argument| argument == STOP_WHEN_DECISIVE);
    let executable = std::env::current_exe().expect("resolve ai_eval executable");

    let outputs = if total_jobs < PROMOTION_PROFILES.len() {
        PROMOTION_PROFILES
            .into_iter()
            .enumerate()
            .map(|(index, profile)| {
                let profile_seed = matrix_profile_seed(seed, index);
                let profile_confirm_seed =
                    confirm_base_seed.map(|prior| matrix_profile_seed(prior, index));
                let mut child_args = matrix_child_args(MatrixChildRequest {
                    challenger,
                    incumbent,
                    pairs,
                    jobs: 1,
                    seed: profile_seed,
                    profile,
                    difficulty: &difficulty,
                    require_artifacts,
                    confirm_seed: profile_confirm_seed,
                });
                if stop_when_decisive {
                    child_args.push(STOP_WHEN_DECISIVE.to_string());
                }
                let output = Command::new(&executable)
                    .args(child_args)
                    .output()
                    .expect("run ai_eval promotion profile");
                (profile, output)
            })
            .collect::<Vec<_>>()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = PROMOTION_PROFILES
                .into_iter()
                .enumerate()
                .map(|(index, profile)| {
                    let executable = executable.clone();
                    let difficulty = difficulty.clone();
                    let jobs = job_budgets[index];
                    scope.spawn(move || {
                        let profile_seed = matrix_profile_seed(seed, index);
                        let profile_confirm_seed =
                            confirm_base_seed.map(|prior| matrix_profile_seed(prior, index));
                        let mut child_args = matrix_child_args(MatrixChildRequest {
                            challenger,
                            incumbent,
                            pairs,
                            jobs,
                            seed: profile_seed,
                            profile,
                            difficulty: &difficulty,
                            require_artifacts,
                            confirm_seed: profile_confirm_seed,
                        });
                        if stop_when_decisive {
                            child_args.push(STOP_WHEN_DECISIVE.to_string());
                        }
                        let output = Command::new(executable)
                            .args(child_args)
                            .output()
                            .expect("run ai_eval promotion profile");
                        (profile, output)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("promotion profile worker panicked"))
                .collect::<Vec<_>>()
        })
    };

    let mut passed = 0usize;
    for (profile, output) in outputs {
        println!("\n===== promotion profile: {} =====", profile.name);
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        let verdict = matrix_verdict(&output.stdout);
        let profile_pass = output.status.success()
            && verdict.is_some_and(|verdict| matrix_profile_accepts(profile.requirement, verdict));
        passed += profile_pass as usize;
        println!(
            "matrix profile result: {} ({:?}) — {} ({:?})",
            profile.name,
            profile.requirement,
            if profile_pass { "ACCEPT" } else { "REJECT" },
            verdict,
        );
    }
    if passed == PROMOTION_PROFILES.len() {
        println!(
            "\nmulti-profile promotion gate: PASS — {challenger} cleared every required profile"
        );
        std::process::exit(0);
    }
    println!(
        "\nmulti-profile promotion gate: RETAIN {incumbent} — {challenger} cleared {passed}/{} required profiles",
        PROMOTION_PROFILES.len()
    );
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--profile <name>` is resolved before anything reads an axis, so every
    // reader below sees the same command line it would have seen from a fully
    // typed-out invocation. With no `--profile` the vector is returned
    // unchanged and this call cannot alter a single recorded result.
    let args = match expand_named_profile(args) {
        Ok(expanded) => expanded,
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(2);
        }
    };
    let a = args.first().map(|name| name.as_str()).unwrap_or("advanced");
    let b = args.get(1).map(|name| name.as_str()).unwrap_or("basic");
    assert_ne!(a, b, "choose two different AIs");
    for name in [a, b] {
        assert!(
            BUILTIN_AIS.contains(&name) || EVAL_ONLY_AIS.contains(&name),
            "unknown AI {name:?}: builtins {BUILTIN_AIS:?}; evaluator-only {EVAL_ONLY_AIS:?}"
        );
    }
    // ★★★★★ WHO ELSE IS ON THE BOARD, AND WHY THAT IS THE WHOLE QUESTION.
    //
    // This evaluator seats the two entrants and nobody else: every one of
    // `--players` chairs is `a` or `b`. Both are `AdvancedAi` variants in
    // practice, `AdvancedAi` routes to religion, and the consequence was
    // measured on 2026-08-18 (`docs/eval/`): over 12 games at the deployment
    // shape the profile produced **9 religious, 3 science, and zero
    // diplomatic, culture or domination**, while the live Civilization VI
    // ladder over the same period lost **41 games to a rival's diplomatic
    // victory and 24 to culture**. The two distributions are nearly disjoint.
    // We screen against ourselves and deploy against somebody who plays
    // differently, and `advanced_congress_counter`,
    // `advanced_congress_votes` and `advanced_congress_counter_hard` have sat
    // unscreened in the registry because the board can never produce the
    // condition they answer. An inert reading would have said nothing.
    //
    // `--field` names the agents that fill the chairs the paired entrants do
    // not take, so a denial treatment can be screened against its incumbent on
    // a board that actually produces the lanes Firaxis' AI pursues. Measured
    // the same day: `--field live_target_diplomatic` turns diplomatic 0 of 12
    // into 3 of 12 and produces culture as well.
    //
    // ⚠ Empty by default, and every existing number in `docs/EVAL.md` was
    // taken with it empty. A field changes the experiment, not the harness: it
    // is a different profile and its results are not comparable to fieldless
    // ones. `--matrix` refuses it for that reason — the promotion matrix's
    // recorded profiles are fieldless.
    let field_names = text(&args, "--field", "");
    let field: Vec<&str> = field_names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    for name in &field {
        if !BUILTIN_AIS.contains(name) && !EVAL_ONLY_AIS.contains(name) {
            eprintln!(
                "--field: unknown AI {name:?}; builtins {BUILTIN_AIS:?}; evaluator-only {EVAL_ONLY_AIS:?}"
            );
            std::process::exit(2);
        }
    }
    if args.iter().any(|argument| argument == "--matrix") {
        run_profile_matrix(&args, a, b);
    }
    // A learned name is only worth recording if its artifacts actually
    // loaded. Say what each entrant resolved to before playing anything, so
    // a result is never filed under an agent that was never in the game.
    let artifact_dir = text(&args, "--artifact-dir", ARTIFACT_DIR);
    // `builtin_provenance`'s contract is "resolve what the production builtin
    // factory will actually construct from `dir`" -- but that factory takes
    // no directory. Every one of its arms resolves the constant `ARTIFACT_DIR`,
    // and the agent constructors below it (`StrategicAi::with_weights` and
    // kin) each load their own net from that same constant. So pointing this
    // flag somewhere
    // else moved the *report* and never the run: it would print a net found in
    // one directory and then play the agent that read another.
    //
    // Reporting a provenance the run does not have is the single failure this
    // whole reporting path exists to prevent, so refuse instead of printing it.
    // Threading a directory into construction is the general fix and is a
    // separate change: it is ~70 call sites inside `elo::builtin_ai`, in a file
    // three open PRs already claim.
    if artifact_dir != ARTIFACT_DIR {
        eprintln!(
            "--artifact-dir {artifact_dir}: unsupported. Agent construction resolves \
             `{ARTIFACT_DIR}` and ignores this flag, so the provenance line would \
             describe a directory this run never reads."
        );
        eprintln!(
            "to evaluate a different artifact, run from a working directory that has \
             it: `{ARTIFACT_DIR}/valuenet.json` or `data/{ARTIFACT_DIR}/valuenet.json`"
        );
        std::process::exit(2);
    }
    // The field is in the game, so it is in the provenance: "a result is never
    // filed under an agent that was never in the game" applies to the chairs
    // that decide which victories are reachable just as much as to the two
    // being compared. Named here rather than only in the profile line so the
    // degraded and `--require-artifacts` checks below cover them as well.
    let mut named: Vec<&str> = vec![a, b];
    for name in &field {
        if !named.contains(name) {
            named.push(name);
        }
    }
    let provenance = builtin_provenances(&named, &artifact_dir);
    for entry in &provenance {
        println!("{}", entry.line());
    }
    let arms = [
        builtin_arm(a).expect("validated left builtin arm"),
        builtin_arm(b).expect("validated right builtin arm"),
    ];
    let axes = arms[0].spec.differing_axes(&arms[1].spec);
    if axes.is_empty() {
        println!("arms differ on: none");
    } else {
        println!("arms differ on: {}", axes.join(", "));
    }
    let collapsed = collapsed_entrants(&[a, b], &artifact_dir);
    for (left, right, shared) in &collapsed {
        println!(
            "warning: {left} and {right} both play as {shared}; this run measures \
             {shared} against itself and says nothing about either name"
        );
    }
    let _ = std::io::stdout().flush();
    if args.iter().any(|arg| arg == "--require-artifacts") {
        let untrained: Vec<&AgentProvenance> = provenance
            .iter()
            .filter(|entry| entry.untrained())
            .collect();
        if !untrained.is_empty() {
            for entry in untrained {
                eprintln!(
                    "{}: missing {} in {artifact_dir}/",
                    entry.requested,
                    entry.missing().join(", ")
                );
            }
            eprintln!("--require-artifacts: refusing to record an untrained result");
            std::process::exit(3);
        }
    } else if !args.iter().any(|arg| arg == "--allow-degraded") {
        let degraded: Vec<&AgentProvenance> = provenance
            .iter()
            .filter(|entry| entry.degraded())
            .collect();
        if !degraded.is_empty() {
            for entry in degraded {
                eprintln!(
                    "{}: requested agent is unavailable and would play as {}",
                    entry.requested, entry.effective
                );
            }
            eprintln!(
                "refusing degraded evaluation by default; name the effective agent or pass --allow-degraded"
            );
            std::process::exit(3);
        }
    }
    let allow_degraded = args.iter().any(|arg| arg == "--allow-degraded");
    if !collapsed.is_empty() {
        eprintln!("refusing to evaluate two names that resolve to one agent");
        std::process::exit(2);
    }
    if axes.len() > 1 && !args.iter().any(|arg| arg == "--deployment-comparison") {
        eprintln!(
            "refusing a {}-axis comparison without --deployment-comparison; this run cannot attribute a replacement result to one mechanism",
            axes.len()
        );
        std::process::exit(2);
    }
    let pairs = number(&args, "--pairs", 50).max(1) as usize;
    // How many games run at once. Every game is independent and seeded, so
    // this changes only wall-clock time, never a result.
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let turns = number(&args, "--turns", 180).max(1) as u32;
    let players = number(&args, "--players", 2).max(2) as usize;
    let city_states = number(&args, "--city-states", 0).max(0) as usize;
    // ⚠ `--city-states` defaults to ZERO, so the stock profile contains no
    // minors at all. An arm whose treatment only exists because city-states do
    // — envoys, influence, suzerainty, the Diplomacy lane — then measures a
    // difference it cannot express, and reports a clean null that means nothing.
    //
    // This is not hypothetical. `advanced_diplomatic_opening` reads zero on the
    // stock profile for exactly this reason, and its lane occupancy only moves
    // once minors are seated: 1.4% -> 2.9% at `--city-states 6`. The prize
    // behind that lane, `Grant::Suzerain`, is the largest the oracle harness has
    // found (56.7% against a 22.7% control, p=0.0000, 400 maps, PR #602), so a
    // silent null here is expensive.
    if city_states == 0 {
        for arm in [a, b] {
            if MINOR_DEPENDENT_ARMS.contains(&arm) {
                println!(
                    "warning: {arm}'s treatment acts through city-states and this profile \
seats none (--city-states 0, the default); it cannot express its difference \
here and any null is uninformative"
                );
            }
        }
    }
    // Every result this evaluator has ever produced was measured at the default
    // game speed, while the exhibition and the live league both run **Online**
    // (`data/speeds.json`: 250 turns, cost_pct 50). A promoted gain is a gain on
    // the game it was measured on, and nothing in this repository has ever
    // checked that one transfers to the other. This flag is what makes that
    // check possible; it defaults to the previous behaviour.
    let speed = text(&args, "--speed", &civvis::game::default_speed());
    // ⚠ A CONTROL THAT CANNOT FINISH TURNS EVERY MARGIN AGAINST IT INTO A
    // READING OF ITSELF. Warned at the profile where the screen was run —
    // Online is the shape the deployment evaluator and the live ladder both
    // play, and `victory_eval` read the opposite answer at Standard/250, which
    // is a Standard game stopped halfway rather than the Online game it looks
    // like. A warning that fired at every speed would be quoting a number
    // outside the profile it was measured on, which is the same mistake.
    if speed.eq_ignore_ascii_case("online") {
        for arm in [a, b] {
            if let Some((_, why)) = DEGENERATE_CONTROLS.iter().find(|(name, _)| *name == arm) {
                println!(
                    "warning: {arm} {why}, so a margin against it measures ITS floor and not the other arm's strength; compare against an arm that finishes, and read a large number here as a broken control rather than a discovery"
                );
            }
        }
    }
    let width = number(&args, "--width", 24).max(8) as i32;
    let height = number(&args, "--height", 16).max(8) as i32;
    let seed = number(&args, "--seed", 4000).max(0) as u64;
    // A gate is a decision procedure. Its point estimate is not an estimator.
    //
    // Every published effect size here is conditioned on having passed a gate,
    // so E[observed | PASS] > true effect — the winner's curse. It shows: +207
    // re-measured to +86, +92 to +61, and `strategic_deep`'s +45 to -8, which
    // #482 excluded outright. Direction and significance replicate; the size
    // fails, always downward. See `docs/EVAL_INTEGRITY.md` §4.
    //
    // `--confirm <prior-seed>` is the claim that this run is the *replication*
    // of one already made elsewhere. Its entire inclusive seed prefix must be
    // disjoint from the discovery prefix; a different base alone can still
    // rerun some of the same maps.
    let confirm_seed = match confirmation_base_seed(&args, seed, pairs) {
        Ok(prior) => prior,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    // The exhibition varies all three world axes and pins its enabled victory
    // set. An evaluator that cannot name them silently measures a different
    // game: historically Pangaea/flat/fixed-roster/all-victories, whatever the
    // command line appeared to say. Keep those historical defaults, but make
    // the deployment axes expressible and print the resolved values below.
    let map_name = text(&args, "--map", MapScript::default().id());
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", MapTopology::default().id());
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", MapPoles::default().id());
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let default_victories = VictoryConditions::NAMES.join(",");
    let victory_names = text(&args, "--victories", &default_victories);
    let victory_conditions = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    });
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    let native_competitions = native_competitions_requested(&args);
    // The difficulty ladder as an external yardstick: the challenger plays
    // the human side of the handicap and its opponents play the AI side, so
    // "beats Emperor" means what a Civ player would expect it to mean.
    // Seats still swap, which moves the challenger around the map rather than
    // moving the handicap.
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    if !Rules::embedded().difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }
    if !field.is_empty() && players < 3 {
        eprintln!(
            "--field needs a chair to sit in: the two entrants take seats 0 and 1, so \
             --players must be at least 3"
        );
        std::process::exit(2);
    }
    let enabled_victories = VictoryConditions::NAMES
        .into_iter()
        .filter(|name| victory_conditions.is_enabled(name))
        .collect::<Vec<_>>()
        .join(",");
    // A named profile is provenance, so it goes at the front of the line the
    // round will quote. An unnamed run says so rather than leaving the reader
    // to assume the axes below happen to match a recorded board.
    let profile_name = text(&args, "--profile", "");
    let named = if profile_name.is_empty() {
        "ad hoc (no --profile)".to_string()
    } else {
        profile_name
    };
    println!(
        "profile: {named}, speed {speed}, map {}, shape {}, poles {}, civilizations {}, victories {enabled_victories}, native competitions {}",
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        if native_competitions {
            "enabled"
        } else {
            "disabled (default)"
        },
    );
    // A result is never filed without saying who was in the game. Printed
    // whether or not a field was named, so a fieldless run states that too
    // rather than leaving the reader to infer it from silence.
    if field.is_empty() {
        println!(
            "field: none — all {players} seats are the paired entrants (every recorded \
             result in docs/EVAL.md was taken this way)"
        );
    } else {
        let seating: Vec<String> = (2..players)
            .map(|pid| format!("{}={}", pid, field[(pid - 2) % field.len()]))
            .collect();
        println!(
            "field: {} — entrants take seats 0 and 1 and swap; the rest are {}",
            field.join(", "),
            seating.join(" ")
        );
        println!(
            "⚠ a field is a different profile: these numbers are not comparable to \
             fieldless runs, and a game won by a field seat is a draw for the pair"
        );
    }
    let mut totals: BTreeMap<String, Metrics> = [a, b]
        .into_iter()
        .chain(field.iter().copied())
        .map(|name| (name.to_string(), Metrics::default()))
        .collect();
    let mut total_turns = 0_u64;
    // ★★★★★ WHICH VICTORY CONDITIONS THIS PROFILE ACTUALLY PRODUCES.
    //
    // A run enables six victories and reports on none of them, so nobody knows
    // which ones it exercised. Measured on the deployment profile with
    // `AdvancedAi` in every seat, 12 games at 250 turns: **9 religious, 3
    // science, and zero diplomatic, culture, domination or score.** The live
    // Civilization VI ladder over the same period lost 41 games to a rival's
    // diplomatic victory and 24 to culture, against 5 religious and 3
    // technology (`docs/EVAL_STATUS.md`).
    //
    // The two distributions are very nearly disjoint, and that has a hard
    // consequence: a treatment aimed at denying a diplomatic or culture
    // victory cannot be screened here, because no such victory happens. It
    // will read as inert, be filed as a null, and the profile will never say
    // that it was the wrong question to ask of it.
    let mut game_victories: BTreeMap<String, usize> = BTreeMap::new();
    let mut pair_scores = Vec::with_capacity(pairs);
    let mut pair_terminal_scores = Vec::with_capacity(pairs);

    // One finished game, carried back from a worker so the fold below can
    // apply it in the order it would have happened serially.
    struct PlayedGame<'a> {
        game: Game,
        seats: Vec<&'a str>,
        challenger_seats: BTreeSet<usize>,
        incumbent_seats: BTreeSet<usize>,
        traces: Vec<PlanTrace>,
        targets: Vec<&'static str>,
        censuses: Vec<Option<ReviewCensus>>,
        expansion_censuses: Vec<Option<ExpansionCensus>>,
        war_reports: Vec<Option<WarPlanReport>>,
    }

    // Games share nothing but the immutable ruleset, and every one is fully
    // determined by its seed, so a batch is embarrassingly parallel. Results
    // come back in index order and are folded sequentially, which makes a
    // parallel run produce byte-identical output to a serial one — only
    // sooner. That matters more here than anywhere else in the codebase:
    // this binary is the promotion gate, and how many maps it can afford is
    // what decides whether an effect is resolvable at all.
    //
    // Chunked rather than one flat batch so peak memory holds a bounded set of
    // finished games rather than the whole run. Four games per worker gives
    // the dynamic scheduler enough slack to hide the large early-victory vs
    // turn-limit cost variance; the former two-game window repeatedly left
    // half the workers idle on each chunk's long tail.
    let chunk_pairs = jobs.max(1).saturating_mul(2);
    let stop_when_decisive = args.iter().any(|arg| arg == STOP_WHEN_DECISIVE);
    let mut stopped_early_at: Option<usize> = None;
    let mut pair = 0usize;
    while pair < pairs {
        let chunk = chunk_pairs.min(pairs - pair);
        let played = civvis::parallel::map(chunk * 2, jobs, |index| {
            let local_pair = pair + index / 2;
            let swap = index % 2;
            let game_seed = seed + local_pair as u64;
            let (seats, challenger_seats, incumbent_seats) = seat_plan(players, swap, a, b, &field);
            let mut game = Game::new_with(GameOptions {
                difficulty: difficulty.clone(),
                human_seats: challenger_seats.clone(),
                speed: speed.clone(),
                map_script,
                map_topology,
                map_poles,
                randomize_civs,
                ..GameOptions::new(players, width, height, game_seed, turns, city_states)
            });
            configure_evaluation_game(&mut game, victory_conditions, native_competitions);
            let mut ais: Vec<Box<dyn Ai>> = game
                .players
                .iter()
                .map(|p| {
                    let name = if p.id < players { seats[p.id] } else { "basic" };
                    evaluator_ai(name, game_seed + p.id as u64, allow_degraded).unwrap_or_else(
                        |error| {
                            panic!(
                                "evaluator preflight permitted an unavailable arm {name:?}: {error}"
                            )
                        },
                    )
                })
                .collect();
            let traces = run_traced_game(&mut game, &mut ais, players);
            let targets = (0..players)
                .map(|pid| plan_target(&game, pid, ais[pid].as_ref()))
                .collect();
            let censuses = (0..players).map(|pid| ais[pid].review_census()).collect();
            let expansion_censuses = (0..players)
                .map(|pid| ais[pid].expansion_census())
                .collect();
            let war_reports = (0..players)
                .map(|pid| ais[pid].plan_report().and_then(|plan| plan.war))
                .collect();
            PlayedGame {
                game,
                seats,
                challenger_seats,
                incumbent_seats,
                traces,
                targets,
                censuses,
                expansion_censuses,
                war_reports,
            }
        });
        for (index, result) in played.into_iter().enumerate() {
            let PlayedGame {
                game,
                seats,
                challenger_seats,
                incumbent_seats,
                traces,
                targets,
                censuses,
                expansion_censuses,
                war_reports,
            } = result;
            total_turns += game.reported_turn() as u64;
            *game_victories
                .entry(
                    game.victory_type
                        .clone()
                        .unwrap_or_else(|| "no winner".to_string()),
                )
                .or_default() += 1;
            let score = game_score(game.winner, &challenger_seats, &incumbent_seats);
            let terminal = terminal_score_share(&game, &challenger_seats, &incumbent_seats);
            if index % 2 == 0 {
                pair_scores.push(score);
                pair_terminal_scores.push(terminal);
            } else {
                *pair_scores.last_mut().expect("the swap follows its pair") += score;
                *pair_terminal_scores
                    .last_mut()
                    .expect("the swap follows its pair") += terminal;
            }
            // Legacy per-seat win metrics count a game nobody won as zero
            // wins. The paired promotion score above records it as a draw.
            for (pid, name) in seats.iter().enumerate() {
                totals
                    .get_mut(*name)
                    .unwrap()
                    .record_war(&traces[pid], war_reports[pid].as_ref());
                if let Some(census) = censuses[pid] {
                    let metrics = totals.get_mut(*name).unwrap();
                    metrics.census.merge(census);
                    metrics.searching_seats += 1;
                }
                if let Some(expansion) = expansion_censuses[pid].as_ref() {
                    let metrics = totals.get_mut(*name).unwrap();
                    metrics.expansion_census.merge(expansion);
                    metrics.expansion_reporting_seats += 1;
                    metrics.dispatch_action_seats +=
                        usize::from(expansion.dispatch_productions > 0);
                    metrics.dispatch_settler_seats +=
                        usize::from(!expansion.dispatch_settler_turns.is_empty());
                    let stock_deadline = game.standard_duration(300).min(
                        game.max_turns
                            .saturating_sub(game.standard_duration(50)),
                    );
                    let late_deadline = game
                        .max_turns
                        .saturating_sub(game.standard_duration(50));
                    metrics.expansion_deadlines = Some((stock_deadline, late_deadline));
                    metrics.dispatch_late_settler_seats += usize::from(
                        expansion
                            .dispatch_settler_turns
                            .iter()
                            .any(|turn| *turn >= stock_deadline && *turn < late_deadline),
                    );
                    metrics.advanced_late_settler_seats += usize::from(
                        !expansion.advanced_late_settler_turns.is_empty(),
                    );
                }
                totals.get_mut(*name).unwrap().record(
                    &game,
                    pid,
                    game.winner == Some(pid),
                    targets[pid],
                    &traces[pid],
                );
            }
        }
        pair += chunk;
        eprintln!("progress: {pair}/{pairs} map pairs complete");
        if stop_when_decisive && pair < pairs {
            // The pair scores are still game+swap sums here; the halving
            // below the loop is what `paired_inference` expects.
            let halved: Vec<f64> = pair_scores.iter().map(|score| score / 2.0).collect();
            if early_stop_is_warranted(&halved) {
                eprintln!(
                    "decisive after {pair} of {pairs} map pairs ({:?}); stopping early under {STOP_WHEN_DECISIVE}",
                    paired_inference(&halved).verdict
                );
                stopped_early_at = Some(pair);
                break;
            }
        }
    }
    for score in pair_scores.iter_mut() {
        *score /= 2.0;
    }
    for score in pair_terminal_scores.iter_mut() {
        *score /= 2.0;
    }

    // Say what was measured, not just how much of it.
    //
    // Nineteen of the twenty `ai_eval` commands recorded in docs/EVAL.md do
    // not specify a map size, so every one of them ran at the 24x16 default
    // and a reader cannot tell without knowing that. The live exhibition now
    // varies dimensions with player count, while the old fixed 74x46-at-six
    // reference was 567 tiles per player against this binary's historical 96.
    // The shipped genome moved `city_target` -40% and `settler_min_pop` +123%,
    // which is the right answer at one density and plausibly the wrong one at
    // the other. Density belongs in the header of anything that claims a
    // strength result.
    let tiles_per_player = (width as f64 * height as f64) / players as f64;
    println!(
        "map: {width}x{height} = {} tiles, {tiles_per_player:.0} per player \
         (live exhibition dimensions vary with player count)",
        width * height
    );
    println!(
        "mirrored head-to-head: {pairs} maps, {} games, {players} players, average {:.1} turns",
        2 * pairs,
        total_turns as f64 / (2 * pairs) as f64
    );
    println!(
        "seed prefix: {seed}..={} (inclusive, one independent map per seed)",
        seed + pairs.saturating_sub(1) as u64
    );
    let games = 2 * pairs;
    print!("game-win share:");
    for name in [a, b] {
        let wins = totals[name].wins;
        print!(
            " {name} {wins}/{games} ({:.1}%)",
            100.0 * wins as f64 / games as f64
        );
    }
    println!();
    if let Some(played) = stopped_early_at {
        println!(
            "stopped early: {played} of {pairs} preregistered map pairs played — the promotion gate's \
             anytime-valid verdict was decisive and {STOP_WHEN_DECISIVE} was given; the sign test and \
             Wilson lines below are read at a data-dependent sample and are not confirmatory here"
        );
    }
    let inference = paired_inference(&pair_scores);
    let outcomes = pair_outcomes(&pair_scores);
    let directions = directional_outcomes(&pair_scores);
    println!(
        "paired-map score for {a}: {:.1}% (95% betting CI {:.1}%..{:.1}%), Elo-equivalent {:+.0} (CI {:+.0}..{:+.0}); \
         the retired maximum-variance Wilson interval on the same maps: {:.1}%..{:.1}%",
        100.0 * inference.score,
        100.0 * inference.low,
        100.0 * inference.high,
        inference.elo,
        inference.elo_low,
        inference.elo_high,
        100.0 * inference.wilson_low,
        100.0 * inference.wilson_high,
    );
    println!(
        "paired outcomes: {a} sweeps {}, neutral splits/draws {}, {b} sweeps {}, draw-mixed {}",
        outcomes.a_sweeps, outcomes.neutral, outcomes.b_sweeps, outcomes.mixed_with_draw
    );
    let sign_p = exact_sign_p(directions.challenger_favored, directions.incumbent_favored);
    let directional_verdict =
        if sign_p < 0.05 && directions.challenger_favored > directions.incumbent_favored {
            format!("SIGNIFICANT {a} DIRECTION")
        } else if sign_p < 0.05 && directions.incumbent_favored > directions.challenger_favored {
            format!("SIGNIFICANT {b} DIRECTION")
        } else {
            "INCONCLUSIVE DIRECTION".to_string()
        };
    println!(
        "paired direction: {a}-favored {}, neutral {}, {b}-favored {}; exact two-sided sign p={sign_p:.4} ({directional_verdict})",
        directions.challenger_favored, directions.neutral, directions.incumbent_favored
    );
    let challenger_crossing = inference
        .anytime
        .challenger_crossed_at
        .map_or("not crossed".to_string(), |map| {
            format!("crossed at map {map}")
        });
    let incumbent_crossing = inference
        .anytime
        .incumbent_crossed_at
        .map_or("not crossed".to_string(), |map| {
            format!("crossed at map {map}")
        });
    println!(
        "anytime-valid betting evidence (2.5% per direction after {PROMOTION_MIN_MAPS} maps): {a} peak e={:.3e}, p<={:.4} ({challenger_crossing}); {b} peak e={:.3e}, p<={:.4} ({incumbent_crossing})",
        inference.anytime.challenger_peak_e,
        inference.anytime.challenger_p,
        inference.anytime.incumbent_peak_e,
        inference.anytime.incumbent_p,
    );
    match inference.verdict {
        PromotionVerdict::Insufficient => println!(
            "promotion gate: INSUFFICIENT — {} independent maps; require at least {PROMOTION_MIN_MAPS}",
            inference.maps
        ),
        PromotionVerdict::Promote => println!(
            "promotion gate: PASS — {a}'s effect interval and anytime-valid evidence both clear parity after {} maps",
            inference.maps,
        ),
        PromotionVerdict::Retain => println!(
            "promotion gate: RETAIN {b} — {b}'s effect interval and anytime-valid evidence both clear parity after {} maps",
            inference.maps,
        ),
        PromotionVerdict::Inconclusive => println!(
            "promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after {} maps",
            inference.maps,
        ),
    }
    // What the verdict above could and could not have seen. Printed for every
    // verdict, not only the inconclusive one: a PASS earned on a run that
    // could barely resolve the edge it reports is also worth knowing about.
    if inference.maps > 0 {
        println!(
            "{}",
            resolution_note(inference.maps, resolved_maps(&directions), RESOLUTION_SEED)
        );
    }
    // Separate the decision from the estimate, in the tool rather than in the
    // discipline. The gate above decides; this line says what the number under
    // it is worth, and it is deliberately printed even when the gate did not
    // fire, so a reader never has to remember which verdicts select on size.
    //
    // The strings here are matched by `unevidenced_effect_sizes` in
    // `tools/civvis_collab.py`: anything this prints must be something that
    // gate already accepts as provenance, or the tool would emit numbers the
    // repository's own docs check then refuses.
    println!(
        "{}",
        effect_size_line(inference.elo, inference.verdict, seed, confirm_seed)
    );
    let terminal_mean = pair_terminal_scores.iter().sum::<f64>() / pairs as f64;
    let terminal_directions = directional_outcomes(&pair_terminal_scores);
    let terminal_sign_p = exact_sign_p(
        terminal_directions.challenger_favored,
        terminal_directions.incumbent_favored,
    );
    let terminal_anytime = anytime_evidence(&pair_terminal_scores);
    println!(
        "paired terminal-score diagnostic for {a}: {:.1}% (not a promotion input)",
        100.0 * terminal_mean
    );
    println!(
        "terminal-score direction: {a}-favored {}, neutral {}, {b}-favored {}; exact two-sided sign p={terminal_sign_p:.4}",
        terminal_directions.challenger_favored,
        terminal_directions.neutral,
        terminal_directions.incumbent_favored,
    );
    // Wins and terminal score are computed from the same games but measure
    // different things: wins count victories, terminal score counts
    // economy. An agent that routes to a victory better wins more games
    // without out-scoring anyone, so the two disagreeing is a finding
    // rather than a fault — it localizes the change to routing rather than
    // development. What must not happen is reading whichever one looks
    // better, so both directions and the number of maps each rests on are
    // reported together.
    let win_resolved = resolved_maps(&directions);
    let terminal_resolved = resolved_maps(&terminal_directions);
    println!(
        "direction resolution: wins rest on {win_resolved} of {} maps that broke, terminal score on {terminal_resolved}",
        inference.maps,
    );
    if !game_victories.is_empty() {
        let total: usize = game_victories.values().sum();
        let mut mix: Vec<(&String, &usize)> = game_victories.iter().collect();
        mix.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
        let detail = mix
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("victory conditions exercised, over all {total} games: {detail}");
        // A condition the profile enabled and never produced is one this run
        // could not have measured a change to. Say so by name.
        let produced: BTreeSet<&str> = game_victories
            .keys()
            .map(|name| name.as_str())
            .filter(|name| *name != "no winner")
            .collect();
        let silent: Vec<&str> = enabled_victories
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty() && !produced.contains(name))
            .collect();
        if !silent.is_empty() {
            println!(
                "⚠ enabled but never produced here: {}. A treatment aimed at one of those cannot \
                 be measured on this profile — an inert reading would say nothing about it",
                silent.join(", ")
            );
        }
        // ★★★ PRODUCED IS NOT THE SAME AS MEASURABLE, AND THE LINE ABOVE ONLY
        // CATCHES EXACTLY ZERO.
        //
        // A lane that decides two games in two hundred and forty passes the
        // silence check and still cannot carry a win-rate read. Only the pairs
        // holding one of those games can turn on the lane, so it can move the
        // paired score by at most `decided / pairs` — and when that ceiling is
        // below the interval this run already reports, the run cannot resolve
        // a change to it however large the change is.
        //
        // This is arithmetic, not a tuned bar: the ceiling and the interval are
        // both numbers the run has already computed. Measured on the 2026-08-19
        // suzerainty round, where `diplomatic` decided 2 of 240 games with a
        // half-width of ~5 points.
        let half_width = (inference.high - inference.low) / 2.0 * 100.0;
        let mut won_by_entrants: BTreeMap<String, usize> = BTreeMap::new();
        for name in [a, b] {
            for (lane, count) in &totals[name].victories {
                *won_by_entrants.entry(lane.clone()).or_default() += count;
            }
        }
        let unresolvable: Vec<String> = unresolvable_lanes(
            &enabled_victories,
            &game_victories,
            &won_by_entrants,
            pairs,
            half_width,
        )
        .into_iter()
        .map(|(name, decided, ceiling)| {
            let won = won_by_entrants.get(&name).copied().unwrap_or(0);
            format!(
                "{name} decided {decided} of {total} games but an entrant won {won} of them, \
                 so it can move the paired score by at most {ceiling:.1} points"
            )
        })
        .collect();
        if !unresolvable.is_empty() {
            println!(
                "⚠ produced but not resolvable here (interval half-width {half_width:.1} points): \
                 {}. A treatment acting through one of those is bounded below this run's own \
                 resolution, so neither a null nor a gain from it means anything",
                unresolvable.join("; ")
            );
        }
    }
    // ★★★★★ NOTHING DIFFERED, WHICH IS NOT THE SAME AS PARITY.
    //
    // Wins break on about a third of maps by construction, so an all-neutral
    // win column is ordinary and says little. Terminal score is continuous and
    // breaks on nearly every map — two agents that play even slightly
    // differently will separate on it somewhere. When *both* columns are
    // neutral on *every* map, the arms did not play close games: they played
    // the same games, and the treatment never changed an outcome on this
    // profile.
    //
    // That is a completely different finding from a null, and it was being
    // reported as one. `advanced_sea_answers` returned 40 of 40 neutral on
    // both columns in the 2026-08-18 triage sweep and read as ordinary parity;
    // the honest reading is that its treatment did not fire. A null asks for a
    // longer screen, this asks for a mechanism check, and buying the former for
    // the latter is how a 200-pair screen gets spent on nothing.
    //
    // ⚠ Deliberately not a `RETAIN`/`Insufficient` verdict change. The gate
    // reports what the evidence supports and this is a note about what the
    // evidence *is*; conflating the two is how the maximum-variance interval
    // came to override the anytime evidence in the first place.
    if inference.maps > 0
        && win_resolved == 0
        && terminal_resolved == 0
        && outcomes.mixed_with_draw == 0
    {
        println!(
            "⚠ nothing differed: all {} maps were neutral on wins AND on terminal score, so {a} and \
             {b} played the same games. The verdict above is not evidence about the treatment — it \
             did not fire on this profile. Check the mechanism before buying a longer run",
            inference.maps,
        );
    }
    match (
        direction_sign(&directions),
        direction_sign(&terminal_directions),
    ) {
        (Some(win), Some(terminal)) if win != terminal => println!(
            "note: wins favour {} and terminal score favours {}. Wins count victories and score counts economy, so this separates victory routing from development rather than contradicting itself",
            if win { a } else { b },
            if terminal { a } else { b },
        ),
        (Some(win), None) => println!(
            "note: wins favour {} while terminal score is flat — a routing change without an economic one",
            if win { a } else { b },
        ),
        _ => {}
    }
    println!(
        "terminal-score anytime evidence (2.5% per direction after {PROMOTION_MIN_MAPS} maps): {a} peak e={:.3e}, p<={:.4}; {b} peak e={:.3e}, p<={:.4}",
        terminal_anytime.challenger_peak_e,
        terminal_anytime.challenger_p,
        terminal_anytime.incumbent_peak_e,
        terminal_anytime.incumbent_p,
    );
    println!("AI          seat-win% score cities pop tech civic dist build military gold");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>7.1}% {:>5.1} {:>6.2} {:>3.1} {:>4.1} {:>5.1} {:>4.1} {:>5.1} {:>8.1} {:>5.1}",
            100.0 * m.wins as f64 / n,
            m.score / n,
            m.cities / n,
            m.population / n,
            m.techs / n,
            m.civics / n,
            m.districts / n,
            m.buildings / n,
            m.military / n,
            m.gold / n,
        );
    }
    println!("\nAI          faith tourists dvp envoys suzerain religious#");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>5.1} {:>8.1} {:>3.1} {:>6.1} {:>8.2} {:>10.2}",
            m.faith / n,
            m.tourists / n,
            m.dvp / n,
            m.envoys / n,
            m.suzerainties / n,
            m.religious_units / n,
        );
    }
    println!("\nAI          mil# civ#  food prod science culture queued-cost");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>4.1} {:>4.1} {:>5.1} {:>4.1} {:>7.1} {:>7.1} {:>11.1}",
            m.military_units / n,
            m.civilian_units / n,
            m.food_yield / n,
            m.production_yield / n,
            m.science_yield / n,
            m.culture_yield / n,
            m.queued_cost / n,
        );
    }
    println!("\nAI          settler builder trader routes/cap support missionary");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>7.2} {:>7.2} {:>6.2} {:>5.2}/{:<4.2} {:>7.2} {:>10.2}",
            m.settlers / n,
            m.builders / n,
            m.traders / n,
            m.active_routes / n,
            m.trade_capacity / n,
            m.support_units / n,
            m.missionaries / n,
        );
    }
    println!("\nExploration:");
    println!(
        "AI          rev@t30 rev@t50 rev@t70 villages v@t50 share meteor wonders 1st-wonder era-score"
    );
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        let mark = |slot: usize| -> String {
            if m.revealed_mark_seats[slot] == 0 {
                "-".to_string()
            } else {
                format!(
                    "{:.0}",
                    m.revealed_at_marks[slot] / m.revealed_mark_seats[slot] as f64
                )
            }
        };
        let early_villages = if m.villages_by_mark_seats == 0 {
            "-".to_string()
        } else {
            format!("{:.2}", m.villages_by_mark / m.villages_by_mark_seats as f64)
        };
        // Mean board endowment over seats; the seat's lifetime claims over it
        // say how much of the contested prize this arm actually won.
        let share = if m.board_village_seats == 0 || m.board_villages == 0.0 {
            "-".to_string()
        } else {
            let board = m.board_villages / m.board_village_seats as f64;
            format!("{:.0}%", 100.0 * (m.goody_huts_claimed / n) / board)
        };
        println!(
            "{name:<11} {:>7} {:>7} {:>7} {:>8.2} {:>5} {:>5} {:>6.2} {:>7.2} {:>10} {:>9.1}",
            mark(0),
            mark(1),
            mark(2),
            m.goody_huts_claimed / n,
            early_villages,
            share,
            m.meteor_goodies_claimed / n,
            m.natural_wonders_discovered / n,
            mean_turn(m.first_wonder_turn_sum, m.first_wonder_seats),
            m.era_score / n,
        );
    }
    println!(
        "AI          minors-met by-t50 1st-minor majors-met by-t50 1st-major recon-peak cities@t60"
    );
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        let tempo = if m.cities_at_t60_seats == 0 {
            "-".to_string()
        } else {
            format!("{:.2}", m.cities_at_t60 / m.cities_at_t60_seats as f64)
        };
        println!(
            "{name:<11} {:>10.2} {:>6.2} {:>9} {:>10.2} {:>6.2} {:>9} {:>10.2} {:>10}",
            m.minors_met / n,
            m.minors_met_by_t50 / n,
            mean_turn(m.first_minor_meet_turn_sum, m.first_minor_meet_seats),
            m.majors_met / n,
            m.majors_met_by_t50 / n,
            mean_turn(m.first_major_meet_turn_sum, m.first_major_meet_seats),
            m.recon_peak / n,
            tempo,
        );
    }
    println!("\nBarbarians:");
    println!("AI          cleared kills lost civ-lost camps<=6@t50 standing");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        let near_home = if m.camps_near_home_seats == 0 {
            "-".to_string()
        } else {
            format!("{:.2}", m.camps_near_home / m.camps_near_home_seats as f64)
        };
        println!(
            "{name:<11} {:>7.2} {:>5.2} {:>4.2} {:>8.2} {:>12} {:>8.2}",
            m.camps_cleared / n,
            m.barbs_killed / n,
            m.lost_to_barbarians / n,
            m.civilians_lost_to_barbarians / n,
            near_home,
            m.camps_standing / n,
        );
    }
    println!("\nVictory types:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].victories);
    }
    println!("\nPlan commitment by observed player-turn:");
    for name in [a, b] {
        let metrics = &totals[name];
        println!(
            "  {name:<11} switches/game {:.2}; {}",
            metrics.plan_switches as f64 / metrics.games.max(1) as f64,
            target_shares(metrics)
        );
    }
    println!("\nAdaptive grand strategy by observed player-turn:");
    // ⚠ A high switch count is NOT by itself a defect, and this line has been
    // misread as one. `leader_study` at the deployment shape (72 games, seeds
    // 14200000.., PR #1572) finds the eventual champion switches MORE than the
    // field, not less: leading on fewest-switches converts at 4-13% against a
    // 17% chance rate, and the champion's mean rank on it is 3.86 of 6 where
    // chance is 3.50. Low churn marks a civ nothing is happening to. Read a
    // difference between the arms here, never the level.
    println!(
        "  (churn LEVEL is not a defect — the champion switches more; \
         compare the arms, not the number)"
    );
    for name in [a, b] {
        let metrics = &totals[name];
        let games = metrics.games.max(1);
        let switches = metrics.midgame_strategy_switches.max(1);
        println!(
            "  {name:<11} all-game switches/seat-game {:.2}, switches/100t {:.2}; {}",
            metrics.strategy_switches as f64 / games as f64,
            100.0 * metrics.strategy_switches as f64 / metrics.plan_observations.max(1) as f64,
            shares(&metrics.strategy_turns, metrics.plan_observations),
        );
        println!(
            "  {name:<11} midgame switches/seat-game {:.2}; unanchored/seat-game {:.2}, {}/{} ({:.1}%); {}",
            metrics.midgame_strategy_switches as f64 / games as f64,
            metrics.midgame_unanchored_switches as f64 / games as f64,
            metrics.midgame_unanchored_switches,
            metrics.midgame_strategy_switches,
            100.0 * metrics.midgame_unanchored_switches as f64 / switches as f64,
            shares(
                &metrics.midgame_strategy_turns,
                metrics.midgame_observations
            ),
        );
        println!(
            "  {name:<11} boundary-accompanied {}/{}; war {}, threat {}, city-deficit {}; transitions {}",
            metrics.midgame_boundary_switches,
            metrics.midgame_strategy_switches,
            metrics.midgame_war_boundary_switches,
            metrics.midgame_threat_boundary_switches,
            metrics.midgame_city_deficit_boundary_switches,
            transition_counts(&metrics.midgame_transitions),
        );
    }
    println!("\nAncient-rush treatment exposure:");
    for name in [a, b] {
        let metrics = &totals[name];
        println!(
            "  {name:<11} {}/{} seat-games ever rushed ({:.1}%); {}/{} observed player-turns rushing ({:.1}%)",
            metrics.rush_seats,
            metrics.games,
            100.0 * metrics.rush_seats as f64 / metrics.games.max(1) as f64,
            metrics.rush_turns,
            metrics.plan_observations,
            100.0 * metrics.rush_turns as f64 / metrics.plan_observations.max(1) as f64,
        );
    }
    if [a, b]
        .iter()
        .any(|name| totals[*name].war_reporting_seats > 0)
    {
        println!("\nUnified timed-war appointment exposure:");
        for name in [a, b] {
            let metrics = &totals[name];
            if metrics.war_reporting_seats == 0 {
                println!("  {name:<11} treatment disabled");
                continue;
            }
            println!(
                "  {name:<11} plans on {}/{} seat-games ({:.1}%); active {}/{} player-turns ({:.1}%)",
                metrics.war_plan_seats,
                metrics.war_reporting_seats,
                100.0 * metrics.war_plan_seats as f64 / metrics.war_reporting_seats.max(1) as f64,
                metrics.war_active_turns,
                metrics.plan_observations,
                100.0 * metrics.war_active_turns as f64 / metrics.plan_observations.max(1) as f64,
            );
            println!(
                "  {name:<11} appointed {}, breakthrough {}, mobilized {}, declared {} (complete {}/{} = {:.1}%), captured {} (within 10t {}/{} = {:.1}%)",
                metrics.war_appointments,
                metrics.war_breakthroughs,
                metrics.war_mobilizations,
                metrics.war_declarations,
                metrics.war_complete_declarations,
                metrics.war_declarations,
                100.0 * metrics.war_complete_declarations as f64
                    / metrics.war_declarations.max(1) as f64,
                metrics.war_objectives_captured,
                metrics.war_objectives_captured_within_ten,
                metrics.war_declarations,
                100.0 * metrics.war_objectives_captured_within_ten as f64
                    / metrics.war_declarations.max(1) as f64,
            );
            println!(
                "  {name:<11} median turns appointment->tech {}, tech->declaration {}, declaration->capture {}; aborts {}",
                median_turns(&metrics.war_appointment_to_tech),
                median_turns(&metrics.war_tech_to_declaration),
                median_turns(&metrics.war_declaration_to_capture),
                transition_counts(
                    &metrics
                        .war_aborts
                        .iter()
                        .map(|(reason, count)| ((*reason).to_string(), *count as usize))
                        .collect()
                ),
            );
        }
    }
    if [a, b]
        .iter()
        .any(|name| totals[*name].expansion_reporting_seats > 0)
    {
        println!("\nAdaptive-expansion production exposure:");
        for name in [a, b] {
            let metrics = &totals[name];
            if metrics.expansion_reporting_seats == 0 {
                println!("  {name:<11} no Advanced-production census to report");
                continue;
            }
            let (stock_deadline, late_deadline) = metrics
                .expansion_deadlines
                .expect("an expansion census records its game-speed deadlines");
            let census = &metrics.expansion_census;
            let dispatch_late_starts = census
                .dispatch_settler_turns
                .iter()
                .filter(|turn| **turn >= stock_deadline && **turn < late_deadline)
                .count();
            println!(
                "  {name:<11} dispatcher calls {}, successful produces {} on {}/{} seats; \
                 Settlers {} on {}/{} seats (late [{stock_deadline},{late_deadline}) {} on {}/{}); \
                 Advanced late Settlers {} on {}/{} seats",
                census.dispatch_calls,
                census.dispatch_productions,
                metrics.dispatch_action_seats,
                metrics.expansion_reporting_seats,
                census.dispatch_settler_turns.len(),
                metrics.dispatch_settler_seats,
                metrics.expansion_reporting_seats,
                dispatch_late_starts,
                metrics.dispatch_late_settler_seats,
                metrics.expansion_reporting_seats,
                census.advanced_late_settler_turns.len(),
                metrics.advanced_late_settler_seats,
                metrics.expansion_reporting_seats,
            );
            println!(
                "  {name:<11} dispatcher Settler turns [{}]; all Advanced late-Settler turns [{}]",
                turn_list(&census.dispatch_settler_turns),
                turn_list(&census.advanced_late_settler_turns),
            );
        }
    }
    // A searching entrant that never reached its search is a scripted agent
    // under a searching agent's name. Say so beside the win rate, because
    // the win rate cannot.
    if [a, b].iter().any(|name| totals[*name].searching_seats > 0) {
        println!("\nMacro search exposure (reviews that reached the rollouts):");
        for name in [a, b] {
            let metrics = &totals[name];
            if metrics.searching_seats == 0 {
                println!("  {name:<11} no search to report");
                continue;
            }
            let census = metrics.census;
            match census.search_exposure() {
                None => println!(
                    "  {name:<11} never reviewed ({} seats; games ended before turn {})",
                    metrics.searching_seats,
                    civvis::strategic::FIRST_REVIEW_TURN
                ),
                Some(share) => println!(
                    "  {name:<11} {}/{} ({:.0}%) reached the rollouts; priors: \
                     duel-religion {}, urgent-counter {}, irreversible-religion {}",
                    census.rollouts,
                    census.total(),
                    100.0 * share,
                    census.duel_religion,
                    census.urgent_counter,
                    census.irreversible_religion
                ),
            }
            if census.joint_reviews > 0 {
                println!(
                    "  {name:<11} joint overrides {}/{} rollout reviews ({:.1}%); lane {}, doctrine {}",
                    census.joint_overrides,
                    census.joint_reviews,
                    100.0 * census.joint_overrides as f64 / census.joint_reviews as f64,
                    census.joint_lane_overrides,
                    census.joint_doctrine_overrides,
                );
            }
        }
        if [a, b]
            .iter()
            .all(|name| totals[*name].searching_seats > 0 && totals[*name].census.rollouts == 0)
        {
            println!(
                "  warning: neither entrant reached its macro search, so this run \
                 compares priors and the scripted parent, not search or evaluator"
            );
        }
    }
    println!("\nFinal plan targets:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].final_targets);
    }
    println!("\nDominant plan targets and seat outcomes:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].dominant_targets);
        for (target, outcome) in &totals[name].target_outcomes {
            println!(
                "    {target:<11} {}/{} wins ({:.1}%)",
                outcome.wins,
                outcome.games,
                100.0 * outcome.wins as f64 / outcome.games.max(1) as f64
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::rng::Rng;

    /// A lane the profile produced twice in two hundred and forty games passes
    /// the "never produced" check and still cannot carry a win-rate read. The
    /// bound is arithmetic: only pairs holding a game the lane decided can turn
    /// on it.
    #[test]
    fn a_lane_too_rare_to_move_the_score_is_named() {
        let mut decided = BTreeMap::new();
        decided.insert("diplomatic".to_string(), 2);
        decided.insert("religious".to_string(), 54);
        let enabled = "science,culture,religious,diplomatic,domination,score";

        // 120 pairs, interval half-width 5 points. Diplomacy tops out at
        // 2/120 = 1.7 points and cannot be seen; religion tops out at 45.
        let won = decided.clone();
        let bounded = unresolvable_lanes(enabled, &decided, &won, 120, 5.0);
        assert_eq!(bounded.len(), 1, "{bounded:?}");
        assert_eq!(bounded[0].0, "diplomatic");
        assert_eq!(bounded[0].1, 2);
        assert!(
            (bounded[0].2 - 5.0 / 3.0).abs() < 1e-9,
            "{:?}",
            bounded[0].2
        );

        // A lane nobody produced belongs to the separate, stronger line.
        assert!(
            !unresolvable_lanes(enabled, &decided, &won, 120, 5.0)
                .iter()
                .any(|(name, ..)| name == "science"),
            "never-produced lanes are reported by the silence check, not this one"
        );

        // Tighten the run and the same lane becomes resolvable.
        assert!(unresolvable_lanes(enabled, &decided, &won, 120, 1.0).is_empty());
        // A run with no pairs has no resolution to compare against.
        assert!(unresolvable_lanes(enabled, &decided, &won, 0, 5.0).is_empty());
    }

    /// The contested board is the case that corrected this. It produced
    /// `diplomatic` 29 times in 240 games — comfortably resolvable if you count
    /// the games — while **neither entrant won one of them**: every single one
    /// went to a scripted field seat, and a game the field wins is a draw for
    /// the pair. The ceiling is therefore zero, not 24 points, and a diplomacy
    /// treatment cannot be screened there by win rate however often the board
    /// produces the lane.
    #[test]
    fn a_lane_only_the_field_ever_wins_cannot_move_the_score() {
        let mut decided = BTreeMap::new();
        decided.insert("diplomatic".to_string(), 29);
        decided.insert("religious".to_string(), 108);
        let mut won = BTreeMap::new();
        won.insert("religious".to_string(), 108);
        let enabled = "science,culture,religious,diplomatic,domination,score";

        let bounded = unresolvable_lanes(enabled, &decided, &won, 120, 3.7);
        assert_eq!(bounded.len(), 1, "{bounded:?}");
        assert_eq!(bounded[0].0, "diplomatic");
        assert_eq!(bounded[0].1, 29, "the games it decided are still reported");
        assert_eq!(bounded[0].2, 0.0, "but none of them can move the pair");
    }

    /// The city-state guard is a list of strings, and lists of strings rot.
    ///
    /// Every name in it must still resolve to a real arm: a rename that left
    /// the old spelling here would silently stop the warning for exactly the
    /// class of arm the list exists to protect, and the failure mode is a
    /// clean, meaningless null rather than an error.
    #[test]
    fn every_minor_dependent_arm_is_still_a_real_arm() {
        for arm in MINOR_DEPENDENT_ARMS {
            assert!(
                builtin_arm(arm).is_some(),
                "{arm} is listed as minor-dependent but is no longer a buildable arm"
            );
        }
    }

    /// `advanced_price_suzerainty` belongs on that list, and the reason is
    /// mechanical: `SUZERAIN_PRIZE` is only ever scored inside the envoy
    /// placement loop, which iterates city-states. Both 400-pair runs that
    /// decided the flag ships off were hand-rolled `ai_eval` lines, and that
    /// path defaults `--city-states` to zero.
    #[test]
    fn the_suzerainty_prize_is_guarded_as_minor_dependent() {
        assert!(MINOR_DEPENDENT_ARMS.contains(&"advanced_price_suzerainty"));
    }

    /// `--stop-when-decisive` ends a run only on a decisive gate: never under
    /// `PROMOTION_MIN_MAPS`, never on parity, and on either side once the
    /// anytime-valid evidence and the betting interval agree.
    #[test]
    fn early_stopping_waits_for_a_decisive_gate() {
        assert!(!early_stop_is_warranted(&[]));
        assert!(
            !early_stop_is_warranted(&[1.0; PROMOTION_MIN_MAPS - 1]),
            "insufficient maps never stop, however lopsided"
        );
        assert!(
            !early_stop_is_warranted(&[0.5; 200]),
            "parity never stops: the run plays out to its preregistered size"
        );
        assert!(
            early_stop_is_warranted(&[1.0; 40]),
            "a challenger sweep stops on PASS"
        );
        assert!(
            early_stop_is_warranted(&[0.0; 40]),
            "an incumbent sweep stops on RETAIN"
        );
        let mixed: Vec<f64> = (0..60)
            .map(|i| if i % 3 == 0 { 0.5 } else { 1.0 })
            .collect();
        assert!(early_stop_is_warranted(&mixed));
        // The verdict the stop reads is the gate's own verdict, not a new rule.
        assert_eq!(paired_inference(&mixed).verdict, PromotionVerdict::Promote);
        assert_eq!(
            paired_inference(&[0.5; 200]).verdict,
            PromotionVerdict::Inconclusive
        );
    }

    #[test]
    fn a_gate_passing_size_is_labelled_the_discovery_estimate_it_is() {
        let line = effect_size_line(207.0, PromotionVerdict::Promote, 4000, None);
        assert!(line.contains("DISCOVERY ESTIMATE"), "{line}");
        assert!(line.contains("biased upward"), "{line}");
        assert!(line.contains("--confirm 4000"), "{line}");

        // RETAIN selects on size in the other direction and is equally biased.
        let retained = effect_size_line(-207.0, PromotionVerdict::Retain, 4000, None);
        assert!(retained.contains("DISCOVERY ESTIMATE"), "{retained}");
    }

    #[test]
    fn a_size_the_gate_did_not_select_says_so_rather_than_claiming_confirmation() {
        for verdict in [
            PromotionVerdict::Inconclusive,
            PromotionVerdict::Insufficient,
        ] {
            let line = effect_size_line(12.0, verdict, 4000, None);
            assert!(line.contains("not gate-selected"), "{line}");
            assert!(!line.contains("DISCOVERY ESTIMATE"), "{line}");
            assert!(!line.contains("CONFIRMED"), "{line}");
        }
    }

    /// Fieldless seating is exactly what it always was.
    ///
    /// Every number in `docs/EVAL.md` was taken through this path, so the
    /// interesting assertion is not that a field works — it is that adding one
    /// changed nothing when nobody asked for it.
    #[test]
    fn a_fieldless_pair_alternates_the_entrants_across_every_chair() {
        for swap in [0, 1] {
            let (seats, challenger, incumbent) = seat_plan(6, swap, "chal", "inc", &[]);
            assert_eq!(seats.len(), 6);
            for (pid, name) in seats.iter().enumerate() {
                let expected = if (pid + swap) % 2 == 0 { "chal" } else { "inc" };
                assert_eq!(*name, expected, "seat {pid} at swap {swap}");
            }
            assert_eq!(challenger.len(), 3, "swap {swap}");
            assert_eq!(incumbent.len(), 3, "swap {swap}");
            // The verdict sets partition the board, which is exactly why the
            // old name test was sufficient here and is not once a field exists.
            assert_eq!(challenger.union(&incumbent).count(), 6, "swap {swap}");
        }
    }

    /// With a field, the entrants hold two chairs and swap between them.
    #[test]
    fn a_field_takes_every_chair_but_the_two_being_compared() {
        let field = ["dip", "cul"];
        let (first, chal_a, inc_a) = seat_plan(6, 0, "chal", "inc", &field);
        assert_eq!(first, vec!["chal", "inc", "dip", "cul", "dip", "cul"]);
        assert_eq!(chal_a.iter().copied().collect::<Vec<_>>(), vec![0]);
        assert_eq!(inc_a.iter().copied().collect::<Vec<_>>(), vec![1]);
        let (second, chal_b, inc_b) = seat_plan(6, 1, "chal", "inc", &field);
        assert_eq!(second, vec!["inc", "chal", "dip", "cul", "dip", "cul"]);
        assert_eq!(chal_b.iter().copied().collect::<Vec<_>>(), vec![1]);
        assert_eq!(inc_b.iter().copied().collect::<Vec<_>>(), vec![0]);
        // Balanced: the challenger holds each of the two chairs exactly once
        // across the pair, and the field is identical in both games.
        assert_eq!(first[2..], second[2..]);
    }

    /// ⚠⚠ A GAME WON BY THE FIELD IS NOT A WIN FOR THE INCUMBENT.
    ///
    /// `game_score` used to read the winner's *name* and score anything that
    /// was not the challenger as `0.0`. That was correct while the two entrants
    /// held every chair and is a defect the moment a field exists: a diplomatic
    /// victory by a `live_target_diplomatic` seat would have counted as a win
    /// for the incumbent — so a denial treatment that failed to stop it would
    /// be penalised, and one that stopped it would gain nothing. The arrangement
    /// makes the arm look worse exactly when it works.
    #[test]
    fn a_victory_by_neither_entrant_is_a_draw_for_the_pair() {
        let field = ["dip", "cul"];
        let (_, challenger, incumbent) = seat_plan(6, 0, "chal", "inc", &field);
        assert_eq!(game_score(Some(0), &challenger, &incumbent), 1.0);
        assert_eq!(game_score(Some(1), &challenger, &incumbent), 0.0);
        for field_seat in 2..6 {
            assert_eq!(
                game_score(Some(field_seat), &challenger, &incumbent),
                0.5,
                "seat {field_seat} is neither entrant"
            );
        }
        assert_eq!(game_score(None, &challenger, &incumbent), 0.5);
        // And the fieldless board still cannot produce that third outcome.
        let (_, chal, inc) = seat_plan(6, 0, "chal", "inc", &[]);
        for pid in 0..6 {
            assert_ne!(
                game_score(Some(pid), &chal, &inc),
                0.5,
                "fieldless seat {pid} belongs to one entrant or the other"
            );
        }
    }

    /// The promotion matrix cannot be handed a different world.
    ///
    /// World rules decide which victories are reachable. The recorded matrix
    /// pins those rules, including its fieldless roster and the default-off
    /// native competition system, so a direct evaluator experiment cannot
    /// silently alter an existing promotion profile.
    #[test]
    fn the_promotion_matrix_owns_every_world_profile_axis() {
        for flag in MATRIX_PROFILE_FLAGS {
            let args = vec![flag.to_string()];
            assert_eq!(
                matrix_profile_flag(&args),
                Some(flag),
                "{flag} must be refused by --matrix"
            );
        }
        assert!(MATRIX_PROFILE_FLAGS.contains(&"--native-competitions"));
        assert_eq!(matrix_profile_flag(&["--pairs".to_string()]), None);
    }

    #[test]
    fn evaluator_applies_native_competitions_to_each_configured_game() {
        let victories = VictoryConditions::parse("diplomatic,score")
            .expect("the fixture names supported victory conditions");
        let mut game = Game::new(2, 20, 14, 52_200, 40, 0);

        assert!(!native_competitions_requested(&[]));
        configure_evaluation_game(&mut game, victories, false);
        assert_eq!(game.victory_conditions, victories);
        assert!(!game.native_competitions);

        let requested = vec!["--native-competitions".to_string()];
        assert!(native_competitions_requested(&requested));
        configure_evaluation_game(&mut game, victories, true);
        assert_eq!(game.victory_conditions, victories);
        assert!(game.native_competitions);
    }

    #[test]
    fn a_confirmation_names_both_seeds_and_is_marked_quotable() {
        let line = effect_size_line(86.0, PromotionVerdict::Promote, 77_200_000, Some(4000));
        assert!(line.contains("CONFIRMED"), "{line}");
        assert!(line.contains("77200000"), "{line}");
        assert!(line.contains("4000"), "{line}");
        assert!(line.contains("quotable"), "{line}");
    }

    /// A control that cannot finish makes every margin against it a reading of
    /// itself, and this evaluator produced exactly that: all four named lanes
    /// "beat" `advanced_target_science` — diplomatic by +669, 23-0-1 — while
    /// diplomatic against religious, both of which finish, is 47.9%, p=1.0000.
    #[test]
    fn a_degenerate_control_is_named_and_says_why() {
        assert!(
            DEGENERATE_CONTROLS
                .iter()
                .any(|(name, _)| *name == "advanced_target_science"),
            "the lane that completes 0/16 is not listed as a degenerate control"
        );
        for (name, why) in DEGENERATE_CONTROLS {
            // A bare list would be a claim; the screen behind each entry is the
            // only thing that makes it checkable by a reader.
            assert!(
                why.contains("victory_eval") || why.contains("games"),
                "{name} is listed without citing the screen that measured it: {why}"
            );
            assert!(
                civvis::elo::builtin_arm(name).is_some(),
                "{name} is listed but is not a selectable arm"
            );
        }
    }

    /// Every entry has to be an arm somebody would reach for as a control,
    /// which in practice means the incumbent of a lane comparison.
    #[test]
    fn a_degenerate_control_is_not_also_the_thing_it_warns_about_measuring() {
        for (name, _) in DEGENERATE_CONTROLS {
            assert!(
                name.starts_with("advanced_"),
                "{name} is not a scripted arm"
            );
        }
    }

    /// The two halves of R3 have to agree, or this tool prints numbers the
    /// repository's own documentation gate then refuses. `EVIDENCE_RE` in
    /// `tools/civvis_collab.py` accepts a seed as provenance; every branch here
    /// names one.
    #[test]
    fn every_effect_size_line_carries_provenance_the_docs_gate_accepts() {
        let verdicts = [
            PromotionVerdict::Promote,
            PromotionVerdict::Retain,
            PromotionVerdict::Inconclusive,
            PromotionVerdict::Insufficient,
        ];
        for verdict in verdicts {
            for confirm in [None, Some(4000)] {
                let line = effect_size_line(45.0, verdict, 90_000, confirm);
                assert!(
                    line.contains("seed"),
                    "no provenance token the docs gate accepts: {line}",
                );
            }
        }
    }

    #[test]
    fn turn_list_is_stable_across_worker_completion_order() {
        assert_eq!(turn_list(&[]), "none");
        assert_eq!(turn_list(&[212, 198, 213, 198]), "198,198,212,213");
    }

    #[test]
    fn promotion_matrix_pins_compact_and_deployment_profiles() {
        let compact = matrix_child_args(MatrixChildRequest {
            challenger: "challenger",
            incumbent: "incumbent",
            pairs: 60,
            jobs: 4,
            seed: 90_000,
            profile: PROMOTION_PROFILES[0],
            difficulty: "prince",
            require_artifacts: false,
            confirm_seed: None,
        });
        let deployment = matrix_child_args(MatrixChildRequest {
            challenger: "challenger",
            incumbent: "incumbent",
            pairs: 60,
            jobs: 4,
            seed: 1_090_000,
            profile: PROMOTION_PROFILES[1],
            difficulty: "prince",
            require_artifacts: false,
            confirm_seed: None,
        });
        for args in [&compact, &deployment] {
            assert_eq!(text(args, "--map", "missing"), "continents");
            assert_eq!(text(args, "--shape", "missing"), "planet");
            assert_eq!(text(args, "--poles", "missing"), "poles");
            assert!(args.iter().any(|argument| argument == "--randomize-civs"));
            assert!(args
                .iter()
                .any(|argument| argument == "--deployment-comparison"));
            assert!(!args.iter().any(|argument| argument == "--matrix"));
        }
        assert_eq!(number(&compact, "--players", 0), 4);
        assert_eq!(number(&compact, "--turns", 0), 500);
        assert_eq!(text(&compact, "--speed", "missing"), "standard");
        assert_eq!(
            text(&compact, "--victories", "missing"),
            "science,culture,domination",
            "the NoRegression tripwire keeps the three-victory set for its measured resolution"
        );
        assert_eq!(number(&deployment, "--players", 0), 6);
        assert_eq!(number(&deployment, "--width", 0), 74);
        assert_eq!(number(&deployment, "--height", 0), 46);
        assert_eq!(number(&deployment, "--turns", 0), 250);
        assert_eq!(text(&deployment, "--speed", "missing"), "online");
        assert_eq!(
            text(&deployment, "--victories", "missing"),
            VictoryConditions::NAMES.join(","),
            "the Strength verdict attaches to the deployment's full victory set"
        );
        assert_eq!(matrix_profile_seed(90_000, 0), 90_000);
        assert_eq!(matrix_profile_seed(90_000, 1), 1_090_000);
        let extended = matrix_child_args(MatrixChildRequest {
            challenger: "challenger",
            incumbent: "incumbent",
            pairs: 120,
            jobs: 4,
            seed: matrix_profile_seed(90_000, 1),
            profile: PROMOTION_PROFILES[1],
            difficulty: "prince",
            require_artifacts: false,
            confirm_seed: None,
        });
        assert_eq!(number(&extended, "--seed", 0), 1_090_000);

        let confirmed = matrix_child_args(MatrixChildRequest {
            challenger: "challenger",
            incumbent: "incumbent",
            pairs: 120,
            jobs: 4,
            seed: matrix_profile_seed(92_000, 1),
            profile: PROMOTION_PROFILES[1],
            difficulty: "prince",
            require_artifacts: false,
            confirm_seed: Some(matrix_profile_seed(90_000, 1)),
        });
        assert_eq!(number(&confirmed, "--seed", 0), 1_092_000);
        assert_eq!(number(&confirmed, "--confirm", 0), 1_090_000);
    }

    #[test]
    fn matrix_confirmation_requires_a_distinct_base_and_preserves_profile_streams() {
        let confirmed = vec![
            "challenger".to_string(),
            "incumbent".to_string(),
            "--matrix".to_string(),
            "--seed".to_string(),
            "82000000".to_string(),
            "--confirm".to_string(),
            "80000000".to_string(),
        ];
        assert_eq!(
            matrix_confirmation_base_seed(&confirmed, 82_000_000, 120),
            Ok(Some(80_000_000))
        );
        assert_eq!(
            matrix_profile_seed(82_000_000, 1),
            83_000_000,
            "the deployment child receives the confirmation stream, not the compact seed"
        );
        assert_eq!(
            matrix_profile_seed(80_000_000, 1),
            81_000_000,
            "the deployment child names its matching discovery stream"
        );

        let same_seed = vec!["--confirm".to_string(), "82000000".to_string()];
        assert!(matrix_confirmation_base_seed(&same_seed, 82_000_000, 120)
            .expect_err("same matrix seed must be rejected")
            .contains("overlaps confirmation"));
    }

    #[test]
    fn confirmation_prefixes_are_disjoint_not_just_different_bases() {
        assert!(disjoint_seed_prefixes(1_000, 1_050, 50).is_ok());
        assert!(disjoint_seed_prefixes(1_050, 1_000, 50).is_ok());

        let overlap = disjoint_seed_prefixes(1_000, 1_025, 50)
            .expect_err("a partial prefix overlap must not count as confirmation");
        assert!(overlap.contains("1000..=1049"), "{overlap}");
        assert!(overlap.contains("1025..=1074"), "{overlap}");

        let overflow = disjoint_seed_prefixes(u64::MAX, 0, 2)
            .expect_err("a prefix that wraps the seed space must be rejected");
        assert!(overflow.contains("overflows u64"), "{overflow}");
    }

    /// The split is stated as invariants rather than a table, because the table
    /// is what broke when a third profile arrived — and a table is the part
    /// nobody reads before adding one.
    #[test]
    fn promotion_matrix_uses_every_worker_and_weights_the_critical_path() {
        let profiles = PROMOTION_PROFILES.len();
        // Fewer workers than profiles: they run sequentially and each child
        // takes the sole worker rather than being handed a fraction of one.
        for jobs in 0..profiles {
            assert_eq!(matrix_job_budgets(jobs), vec![1; profiles], "{jobs} jobs");
        }
        for jobs in profiles..64 {
            let budgets = matrix_job_budgets(jobs);
            assert_eq!(budgets.len(), profiles, "{jobs} jobs");
            // Every requested worker is used, and none is given to nobody.
            assert_eq!(
                budgets.iter().sum::<usize>(),
                jobs,
                "{jobs} jobs: {budgets:?}"
            );
            assert!(
                budgets.iter().all(|budget| *budget > 0),
                "{jobs} jobs: {budgets:?}"
            );
            // A heavier profile is never given less than a lighter one: the
            // matrix's wall time is the slowest child, so starving the
            // expensive shape is the one allocation that costs real time.
            for (index, budget) in budgets.iter().enumerate() {
                for (other, other_budget) in budgets.iter().enumerate() {
                    if PROMOTION_PROFILES[index].cost_weight > PROMOTION_PROFILES[other].cost_weight
                    {
                        assert!(
                            budget >= other_budget,
                            "{jobs} jobs: {} got {budget} and the lighter {} got {other_budget}",
                            PROMOTION_PROFILES[index].name,
                            PROMOTION_PROFILES[other].name
                        );
                    }
                }
            }
        }
        // The deployment shape really is the heavy one, so the invariant above
        // is pointed at the right profile rather than being vacuously true.
        assert!(PROMOTION_PROFILES
            .iter()
            .any(|profile| profile.cost_weight > 1));
    }

    /// ★★★★★ THE HAND-TYPED CONTESTED ROUNDS WERE NOT ON THE CONTESTED BOARD.
    ///
    /// Four rounds in `docs/eval/` measured congress arms on
    /// `--field live_target_diplomatic,live_target_culture` and each states its
    /// world as "`pangaea`/`flat`/fixed civilizations". `deployment-contested`,
    /// the profile the promotion gate runs, has been `continents`/`planet`/
    /// `poles`/randomized since #658. Eleven flags typed by hand agreed with
    /// the gate on eight of them, and nothing in the repository could notice.
    ///
    /// This is the check that makes `--profile <name>` worth having: the name
    /// and the matrix child must expand to one world, derived from one place.
    #[test]
    fn a_named_profile_plays_the_same_world_as_its_matrix_child() {
        for profile in PROMOTION_PROFILES {
            let child = matrix_child_args(MatrixChildRequest {
                challenger: "chal",
                incumbent: "inc",
                pairs: 60,
                jobs: 4,
                seed: 90_000,
                profile,
                difficulty: "prince",
                require_artifacts: false,
                confirm_seed: None,
            });
            let named = expand_named_profile(vec![
                "chal".to_string(),
                "inc".to_string(),
                "--profile".to_string(),
                profile.name.to_string(),
            ])
            .unwrap_or_else(|why| panic!("{} should resolve: {why}", profile.name));
            for flag in ["--players", "--width", "--height", "--city-states", "--turns"] {
                assert_eq!(
                    number(&named, flag, -1),
                    number(&child, flag, -2),
                    "{} disagrees on {flag}",
                    profile.name
                );
            }
            for flag in ["--speed", "--map", "--shape", "--poles", "--victories"] {
                assert_eq!(
                    text(&named, flag, "named-missing"),
                    text(&child, flag, "child-missing"),
                    "{} disagrees on {flag}",
                    profile.name
                );
            }
            for flag in ["--randomize-civs", "--field"] {
                assert_eq!(
                    named.iter().any(|argument| argument == flag),
                    child.iter().any(|argument| argument == flag),
                    "{} disagrees on {flag}",
                    profile.name
                );
            }
            assert_eq!(
                text(&named, "--field", ""),
                profile.field,
                "{} did not seat its field",
                profile.name
            );
            // The name survives into the resolved command line, because the
            // run has to be able to report which board it played.
            assert_eq!(text(&named, "--profile", ""), profile.name);
            // `--deployment-comparison` belongs to the matrix's replacement
            // question, not to the world, so a named single-arm run does not
            // silently inherit permission for a multi-axis comparison.
            assert!(!named
                .iter()
                .any(|argument| argument == "--deployment-comparison"));
        }
    }

    /// A profile that can be silently overridden is a profile that reports one
    /// world and plays another — the defect `--artifact-dir` is refused for.
    #[test]
    fn a_named_profile_refuses_the_axes_it_already_fixes() {
        let unchanged = vec!["chal".to_string(), "inc".to_string(), "--pairs".to_string()];
        assert_eq!(
            expand_named_profile(unchanged.clone()).expect("no --profile is a no-op"),
            unchanged,
            "a run with no --profile must be returned byte-identical"
        );

        for flag in MATRIX_PROFILE_FLAGS {
            let why = expand_named_profile(vec![
                "chal".to_string(),
                "inc".to_string(),
                "--profile".to_string(),
                "deployment-contested".to_string(),
                flag.to_string(),
                "7".to_string(),
            ])
            .expect_err("an explicitly named axis must be refused, not overridden");
            assert!(why.contains(flag), "{why}");
        }

        let matrixed = expand_named_profile(vec![
            "chal".to_string(),
            "inc".to_string(),
            "--profile".to_string(),
            "deployment-contested".to_string(),
            "--matrix".to_string(),
        ])
        .expect_err("--profile and --matrix are different questions");
        assert!(matrixed.contains("--matrix"), "{matrixed}");

        let unknown = expand_named_profile(vec![
            "chal".to_string(),
            "inc".to_string(),
            "--profile".to_string(),
            "deployment-contsted".to_string(),
        ])
        .expect_err("a misspelled profile must not silently fall back to the defaults");
        assert!(unknown.contains("deployment-contested"), "{unknown}");

        let nameless = expand_named_profile(vec![
            "chal".to_string(),
            "inc".to_string(),
            "--profile".to_string(),
        ])
        .expect_err("--profile with no name must be refused");
        assert!(nameless.contains("deployment-contested"), "{nameless}");
    }

    /// ⚠⚠ THE GATE MUST BE ABLE TO PRODUCE THE VICTORIES IT IS GATING ON.
    ///
    /// Every promotion decision in this repository's history was taken on two
    /// profiles that seat `AdvancedAi` in every chair. `AdvancedAi` routes to
    /// religion, and the deployment profile was measured producing **zero
    /// diplomatic and zero culture victories over 40 games**, twice, on two
    /// disjoint seed streams — while the live Civilization VI ladder lost 41
    /// games to a rival's diplomatic victory and 24 to culture. This pins the
    /// repair: at least one profile is contested, it is the deployment shape,
    /// and it is a tripwire rather than a third hurdle.
    #[test]
    fn one_promotion_profile_is_contested_and_is_a_tripwire() {
        let contested: Vec<&MatrixProfile> = PROMOTION_PROFILES
            .iter()
            .filter(|profile| !profile.field.is_empty())
            .collect();
        assert_eq!(
            contested.len(),
            1,
            "exactly one profile should carry a field; the others are the recorded fieldless ones"
        );
        let contested = contested[0];
        // The lanes the front line actually loses to have to be the ones seated.
        for lane in ["live_target_diplomatic", "live_target_culture"] {
            assert!(
                contested.field.contains(lane),
                "{} does not seat {lane}",
                contested.name
            );
            assert!(
                EVAL_ONLY_AIS.contains(&lane) || BUILTIN_AIS.contains(&lane),
                "{lane} is not a constructible agent"
            );
        }
        // Same board as deployment, so the only difference is the company.
        let deployment = PROMOTION_PROFILES
            .iter()
            .find(|profile| profile.name == "deployment-online")
            .expect("the deployment profile is still named that");
        assert_eq!(contested.players, deployment.players);
        assert_eq!(contested.width, deployment.width);
        assert_eq!(contested.height, deployment.height);
        assert_eq!(contested.city_states, deployment.city_states);
        assert_eq!(contested.turns, deployment.turns);
        assert_eq!(contested.speed, deployment.speed);
        assert_eq!(contested.victories, deployment.victories);
        // A tripwire, not a hurdle: an inconclusive reading here must not block
        // a promotion, and a measured regression must.
        assert_eq!(contested.requirement, MatrixRequirement::NoRegression);
        assert!(matrix_profile_accepts(
            contested.requirement,
            MatrixVerdict::Inconclusive
        ));
        assert!(!matrix_profile_accepts(
            contested.requirement,
            MatrixVerdict::Retain
        ));
        // And the child invocation actually carries the field, which is the
        // step that would silently make this whole profile a duplicate of
        // `deployment-online`.
        let args = matrix_child_args(MatrixChildRequest {
            challenger: "challenger",
            incumbent: "incumbent",
            pairs: 60,
            jobs: 4,
            seed: 2_090_000,
            profile: *contested,
            difficulty: "prince",
            require_artifacts: false,
            confirm_seed: None,
        });
        assert_eq!(text(&args, "--field", "missing"), contested.field);
        for profile in PROMOTION_PROFILES.iter().filter(|p| p.field.is_empty()) {
            let fieldless = matrix_child_args(MatrixChildRequest {
                challenger: "challenger",
                incumbent: "incumbent",
                pairs: 60,
                jobs: 4,
                seed: 90_000,
                profile: *profile,
                difficulty: "prince",
                require_artifacts: false,
                confirm_seed: None,
            });
            assert!(
                !fieldless.iter().any(|argument| argument == "--field"),
                "{} is a recorded fieldless profile and must stay one",
                profile.name
            );
        }
    }

    #[test]
    fn promotion_matrix_requires_strength_at_deployment_and_safety_elsewhere() {
        assert_eq!(
            matrix_verdict(b"promotion gate: PASS \xe2\x80\x94 challenger cleared"),
            Some(MatrixVerdict::Pass)
        );
        assert_eq!(
            matrix_verdict(b"promotion gate: RETAIN incumbent \xe2\x80\x94 regression"),
            Some(MatrixVerdict::Retain)
        );
        assert!(matrix_profile_accepts(
            MatrixRequirement::Strength,
            MatrixVerdict::Pass
        ));
        assert!(!matrix_profile_accepts(
            MatrixRequirement::Strength,
            MatrixVerdict::Inconclusive
        ));
        assert!(matrix_profile_accepts(
            MatrixRequirement::NoRegression,
            MatrixVerdict::Inconclusive
        ));
        assert!(!matrix_profile_accepts(
            MatrixRequirement::NoRegression,
            MatrixVerdict::Retain
        ));
        assert!(!matrix_profile_accepts(
            MatrixRequirement::NoRegression,
            MatrixVerdict::Insufficient
        ));
    }

    /// A 95% interval for the mean of paired map scores that uses their
    /// observed variance instead of the worst case.
    ///
    /// The Wilson interval the promotion gate turns on treats every map as a
    /// maximum-variance Bernoulli draw. That is the right worst case for a
    /// coin and the wrong one here: a mirrored A/B between close agents splits
    /// most maps, and a split scores exactly 0.5, so the realised per-map
    /// variance is a small fraction of the assumed `p(1-p)`. On a 120-map run
    /// where 103 maps split, the observed variance is about 0.05 against
    /// Wilson's assumed 0.25, and Wilson spans 46.5%..64.0% around a 55.4%
    /// mean — unable to clear parity however consistent the decisive maps are.
    ///
    /// This is the ordinary normal interval on the sample variance. The
    /// non-asymptotic alternative for bounded variables, the empirical
    /// Bernstein bound, was tried first and rejected by measurement: its
    /// additive `3 ln(3/delta) / n` term dominates at these sample sizes and
    /// made the interval *wider* than Wilson (0.32 against 0.18 at n=120), so
    /// it pays for its worst-case guarantee with exactly the width this is
    /// meant to remove. Coverage is therefore established by simulation rather
    /// than by a finite-sample proof — see
    /// `the_variance_adaptive_interval_covers_the_null_it_claims`, which
    /// checks it against the map shapes these runs actually produce.
    fn bootstrap_interval(scores: &[f64], seed: u64) -> (f64, f64) {
        let n = scores.len();
        if n < 2 {
            return (0.0, 1.0);
        }
        let mut rng = civvis::rng::Rng::new(seed);
        let mut means: Vec<f64> = (0..BOOTSTRAP_RESAMPLES)
            .map(|_| (0..n).map(|_| scores[rng.below(n)]).sum::<f64>() / n as f64)
            .collect();
        means.sort_by(|a, b| a.partial_cmp(b).expect("means are finite"));
        let low = means[(BOOTSTRAP_RESAMPLES as f64 * 0.025) as usize];
        let high =
            means[((BOOTSTRAP_RESAMPLES as f64 * 0.975) as usize).min(BOOTSTRAP_RESAMPLES - 1)];
        (low, high)
    }

    const BOOTSTRAP_RESAMPLES: usize = 2000;

    fn variance_adaptive_interval(scores: &[f64]) -> (f64, f64) {
        let n = scores.len();
        if n < 2 {
            return (0.0, 1.0);
        }
        let count = n as f64;
        let mean = scores.iter().sum::<f64>() / count;
        let variance = scores
            .iter()
            .map(|score| (score - mean) * (score - mean))
            .sum::<f64>()
            / (count - 1.0);
        let radius = Z_95 * (variance / count).sqrt();
        (
            (mean - radius).clamp(0.0, 1.0),
            (mean + radius).clamp(0.0, 1.0),
        )
    }

    #[test]
    fn confidence_uses_mirrored_maps_as_independent_observations() {
        let one_map = paired_inference(&[1.0]);
        let two_maps = paired_inference(&[1.0, 1.0]);
        assert_eq!(one_map.maps, 1);
        // Wilson narrows on the second map because its width is a function of
        // the count alone. The gate's interval does not, and should not: two
        // maps cannot exclude any mean at 2.5% per side, so an interval that
        // claimed otherwise would be claiming evidence the run has not got.
        assert!(one_map.wilson_low < two_maps.wilson_low);
        assert_eq!((one_map.low, one_map.high), (0.0, 1.0));
        assert_eq!((two_maps.low, two_maps.high), (0.0, 1.0));
        assert!(one_map.high <= 1.0);
        assert_eq!(one_map.verdict, PromotionVerdict::Insufficient);
        // Replication is what narrows it, and it narrows a long way.
        let twenty = paired_inference(&[1.0; PROMOTION_MIN_MAPS]);
        let forty = paired_inference(&[1.0; 2 * PROMOTION_MIN_MAPS]);
        assert!(twenty.low > 0.5);
        assert!(forty.low > twenty.low);
    }

    #[test]
    fn strong_replicated_edge_passes_promotion_gate() {
        let scores = vec![1.0; 30];
        let result = paired_inference(&scores);
        assert!(result.low > 0.5);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(
            result.anytime.challenger_crossed_at,
            Some(PROMOTION_MIN_MAPS)
        );
        assert_eq!(result.verdict, PromotionVerdict::Promote);
    }

    #[test]
    fn minimum_map_gate_overrides_an_early_clean_sweep() {
        let result = paired_inference(&[1.0; PROMOTION_MIN_MAPS - 1]);
        assert!(result.low > 0.5);
        assert_eq!(result.anytime.challenger_peak_e, 1.0);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Insufficient);
    }

    #[test]
    fn decisive_incumbent_edge_retains_it() {
        let result = paired_inference(&vec![0.0; 30]);
        assert!(result.high < 0.5);
        assert!(result.anytime.incumbent_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.verdict, PromotionVerdict::Retain);
    }

    #[test]
    fn balanced_maps_are_inconclusive() {
        let scores: Vec<f64> = (0..40)
            .map(|index| if index % 2 == 0 { 1.0 } else { 0.0 })
            .collect();
        let result = paired_inference(&scores);
        assert!(result.low < 0.5 && result.high > 0.5);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn neutral_maps_neither_spend_nor_create_betting_evidence() {
        let result = paired_inference(&vec![0.5; 100]);
        assert_eq!(result.anytime.challenger_peak_e, 1.0);
        assert_eq!(result.anytime.incumbent_peak_e, 1.0);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn repeated_draw_mixed_edges_accumulate_bounded_score_evidence() {
        let result = paired_inference(&vec![0.75; 80]);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Promote);
    }

    #[test]
    fn subminimum_lucky_prefix_cannot_bank_a_later_promotion() {
        let mut scores = vec![1.0; PROMOTION_MIN_MAPS / 2];
        scores.extend(vec![0.0; PROMOTION_MIN_MAPS / 2]);
        let result = paired_inference(&scores);
        assert_eq!(result.anytime.challenger_crossed_at, None);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn contradictory_anytime_crossings_flag_nonstationarity() {
        let mut scores = vec![1.0; 30];
        scores.extend(vec![0.0; 100]);
        let result = paired_inference(&scores);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert!(result.anytime.incumbent_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn elo_equivalent_is_symmetric_around_parity() {
        assert!((elo_edge(0.64) + elo_edge(0.36)).abs() < 1e-9);
        assert_eq!(elo_edge(0.5), 0.0);
    }

    #[test]
    fn pair_outcome_counts_keep_draw_mixed_maps_visible() {
        assert_eq!(
            pair_outcomes(&[1.0, 0.5, 0.0, 0.25, 0.75]),
            PairOutcomes {
                a_sweeps: 1,
                neutral: 1,
                b_sweeps: 1,
                mixed_with_draw: 2,
            }
        );
    }

    #[test]
    fn games_without_a_head_to_head_winner_are_draws() {
        let challenger = BTreeSet::from([0]);
        let incumbent = BTreeSet::from([1]);
        assert_eq!(game_score(Some(0), &challenger, &incumbent), 1.0);
        assert_eq!(game_score(Some(1), &challenger, &incumbent), 0.0);
        assert_eq!(game_score(None, &challenger, &incumbent), 0.5);
        assert_eq!(game_score(Some(2), &challenger, &incumbent), 0.5);
    }

    /// A threaded batch must produce the serial numbers exactly. Every game
    /// is determined by its seed and shares nothing mutable, and results are
    /// folded in index order, so the only thing `--jobs` may change is how
    /// long the run takes. If this ever fails, the evaluator has started
    /// reporting a different answer depending on the machine it ran on.
    /// A direction is only as strong as the number of maps that broke. The
    /// win statistic and the terminal-score statistic run on the same games
    /// and routinely rest on very different map counts, which is the fact
    /// that stops a 5-0 win margin from five maps reading as stronger than
    /// a 10-8 score margin from eighteen.
    /// Why the gate's conservatism was measured and then left alone.
    ///
    /// Under the null — mean exactly 0.5, with the map shape these runs
    /// produce — a 95% interval should contain 0.5 in about 95% of
    /// replications. Wilson contains it in *all* of them at 2.2x the width
    /// of the alternatives, and both natural alternatives land near 93.5%,
    /// slightly under nominal. So the conservatism is real and there is no
    /// drop-in replacement that is both narrower and calibrated. Recorded
    /// as an experiment rather than shipped as a statistic.
    #[test]
    fn the_fast_verdict_agrees_with_the_full_one() {
        let mut rng = Rng::new(4_242);
        for _ in 0..200 {
            let maps = PROMOTION_MIN_MAPS + rng.below(40);
            let scores: Vec<f64> = (0..maps)
                .map(|_| match rng.below(10) {
                    0..=2 => 1.0,
                    3 => 0.0,
                    _ => 0.5,
                })
                .collect();
            assert_eq!(
                gate_would_promote(&scores),
                paired_inference(&scores).verdict == PromotionVerdict::Promote,
                "fast and full verdicts disagree on {scores:?}"
            );
        }
    }

    /// The number this prints is the whole point, so pin its shape: more maps
    /// must resolve a smaller edge, and the counts this repository actually
    /// runs at must land either side of the effects it actually produces.
    /// The all-neutral case has to be distinguishable from close play, and the
    /// distinction is that terminal score is continuous: agents that play even
    /// slightly differently separate on it somewhere.
    #[test]
    fn identical_games_are_distinguishable_from_close_ones() {
        // Two agents that played the same games: every map neutral on both.
        let identical = vec![0.5; 30];
        assert_eq!(resolved_maps(&directional_outcomes(&identical)), 0);
        assert_eq!(pair_outcomes(&identical).mixed_with_draw, 0);
        assert_eq!(pair_outcomes(&identical).neutral, 30);

        // Close play: the win column is all-neutral, which is ordinary, but
        // terminal score separates. This must NOT read as "nothing differed".
        let wins = vec![0.5; 30];
        let terminal: Vec<f64> = (0..30)
            .map(|index| if index % 2 == 0 { 0.55 } else { 0.45 })
            .collect();
        assert_eq!(resolved_maps(&directional_outcomes(&wins)), 0);
        assert_eq!(resolved_maps(&directional_outcomes(&terminal)), 30);

        // And a draw-mixed map is a difference even when neither column
        // resolves a direction, so it also blocks the claim.
        let mixed = vec![0.75, 0.5, 0.5];
        assert_eq!(pair_outcomes(&mixed).mixed_with_draw, 1);
    }

    #[test]
    fn the_reported_resolution_tightens_with_map_count() {
        let break_rate = 0.28;
        let forty = resolvable_edge(40, break_rate, RESOLUTION_SEED);
        let two_hundred = resolvable_edge(200, break_rate, RESOLUTION_SEED);
        let (forty, two_hundred) = (
            forty.expect("40 maps resolves something"),
            two_hundred.expect("200 maps resolves something"),
        );
        println!("40 maps: {forty:+.0} Elo   200 maps: {two_hundred:+.0} Elo");
        assert!(
            two_hundred < forty,
            "more maps must resolve a smaller edge: {two_hundred:+.0} against {forty:+.0}"
        );
        // The finding this exists to make visible: a 40-map run cannot see the
        // size of edge this repository's changes actually produce, so an
        // INCONCLUSIVE there is not evidence of absence.
        assert!(
            forty > 60.0,
            "40 maps was expected to be unable to resolve a small edge, got {forty:+.0}"
        );
        assert!(
            two_hundred < forty * 0.8,
            "200 maps should be materially better than 40: {two_hundred:+.0} against {forty:+.0}"
        );
    }

    #[test]
    fn a_run_under_the_map_floor_reports_no_resolution() {
        assert_eq!(
            resolvable_edge(PROMOTION_MIN_MAPS - 1, 0.3, RESOLUTION_SEED),
            None
        );
        assert!(resolution_note(10, 3, RESOLUTION_SEED).contains("too few"));
    }

    /// A run where nothing broke has no break rate to reason from, and must
    /// say so rather than quoting a number derived from a division by zero.
    #[test]
    fn a_run_with_no_broken_maps_reports_no_resolution() {
        let note = resolution_note(40, 0, RESOLUTION_SEED);
        assert!(note.contains("too few"), "{note}");
    }

    #[test]
    fn the_gate_resolves_the_edges_it_used_to_call_inconclusive() {
        // The three paired-map score vectors this repository has actually
        // recorded and filed as inconclusive, reconstructed from the pair
        // outcomes in docs/EVAL.md. Every one of them is a run where the
        // maximum-variance interval, not the evidence, was the binding
        // constraint: its lower bound sat far under parity while the mean
        // stood at +89 to +100 Elo-equivalent.
        // The fourth field is whether the betting lower bound is expected to
        // beat Wilson's. It does not at exactly the 20-map floor, where only
        // one prefix is monitored and the mixture still pays for the bets the
        // run did not need; it does from 25 maps up, and the margin grows.
        // Deployment runs are decided at 40 maps and above.
        let recorded: [(&str, usize, usize, usize, bool); 3] = [
            ("advanced vs basic", 6, 13, 1, false),
            ("deployment 36-map", 6, 29, 1, true),
            ("strategic 25-map", 8, 16, 1, true),
        ];
        for (name, favored, neutral, against, tighter) in recorded {
            let scores: Vec<f64> = std::iter::repeat_n(1.0, favored)
                .chain(std::iter::repeat_n(0.5, neutral))
                .chain(std::iter::repeat_n(0.0, against))
                .collect();
            let inference = paired_inference(&scores);
            println!(
                "{name}: n={} mean={:.3} betting [{:.3},{:.3}] wilson [{:.3},{:.3}]",
                inference.maps,
                inference.score,
                inference.low,
                inference.high,
                inference.wilson_low,
                inference.wilson_high
            );
            assert!(
                inference.wilson_low < 0.5,
                "{name}: the retired interval is expected to have blocked this run"
            );
            assert_eq!(
                inference.low > inference.wilson_low,
                tighter,
                "{name}: betting low {:.3} against wilson low {:.3}",
                inference.low,
                inference.wilson_low
            );
        }
    }

    /// The gate's promise is 2.5% per direction. Measure what it actually
    /// spends when the two arms are the same agent, on the map shape these
    /// runs produce, so a later widening of the bet grid cannot quietly turn
    /// a conservative gate into a permissive one.
    #[test]
    fn the_promotion_gate_stays_inside_its_declared_error_budget() {
        let mut rng = Rng::new(20_260_818);
        let trials = 600;
        let mut promoted = 0;
        let mut retained = 0;
        for _ in 0..trials {
            let scores: Vec<f64> = (0..40)
                .map(|_| match rng.below(20) {
                    0..=3 => 1.0,
                    4..=7 => 0.0,
                    _ => 0.5,
                })
                .collect();
            match paired_inference(&scores).verdict {
                PromotionVerdict::Promote => promoted += 1,
                PromotionVerdict::Retain => retained += 1,
                _ => {}
            }
        }
        println!("null verdicts: {promoted} promote, {retained} retain of {trials}");
        assert!(
            promoted * 40 <= trials,
            "promotion spent more than 2.5% of its null budget: {promoted}/{trials}"
        );
        assert!(
            retained * 40 <= trials,
            "retention spent more than 2.5% of its null budget: {retained}/{trials}"
        );
    }

    #[test]
    fn a_betting_interval_is_the_narrower_calibrated_one() {
        let mut rng = Rng::new(20_260_726);
        let mut covered = 0;
        let mut betting_covered = 0;
        let mut wilson_covered = 0;
        let mut eb_width = 0.0;
        let mut boot_covered = 0;
        let mut boot_width = 0.0;
        let mut betting_width = 0.0;
        let mut wilson_width = 0.0;
        let trials = 400;
        let maps = 120;
        for _ in 0..trials {
            // The observed shape: most maps split, the rest break evenly
            // either way, so the mean is 0.5 under the null.
            let scores: Vec<f64> = (0..maps)
                .map(|_| match rng.below(10) {
                    0 => 1.0,
                    1 => 0.0,
                    _ => 0.5,
                })
                .collect();
            let (low, high) = variance_adaptive_interval(&scores);
            if low <= 0.5 && 0.5 <= high {
                covered += 1;
            }
            eb_width += high - low;
            let (blow, bhigh) = bootstrap_interval(&scores, 900 + covered as u64);
            if blow <= 0.5 && 0.5 <= bhigh {
                boot_covered += 1;
            }
            boot_width += bhigh - blow;
            let inference = paired_inference(&scores);
            if inference.low <= 0.5 && 0.5 <= inference.high {
                betting_covered += 1;
            }
            betting_width += inference.high - inference.low;
            if inference.wilson_low <= 0.5 && 0.5 <= inference.wilson_high {
                wilson_covered += 1;
            }
            wilson_width += inference.wilson_high - inference.wilson_low;
        }
        println!(
            "normal/sample-variance: {covered}/{trials} covered, mean width {:.4}",
            eb_width / trials as f64
        );
        println!(
            "bootstrap percentile:   {boot_covered}/{trials} covered, mean width {:.4}",
            boot_width / trials as f64
        );
        println!(
            "betting (shipped):      {betting_covered}/{trials} covered, mean width {:.4}",
            betting_width / trials as f64
        );
        println!(
            "wilson (retired):       {wilson_covered}/{trials} covered, mean width {:.4}",
            wilson_width / trials as f64
        );
        // The finding this test used to pin was that no drop-in narrower
        // interval here is also calibrated: Wilson covered every replication
        // at 2.2x the width of the two variance-adaptive alternatives, and
        // both of those landed *under* nominal. The first half still holds and
        // is asserted below. The conclusion does not, and the reason it did
        // not is worth keeping: both alternatives estimate a dispersion from
        // 120 observations that are mostly the same number, and an interval
        // built on an underestimated variance undercovers.
        //
        // A betting interval never estimates that dispersion. It inverts the
        // e-process the gate already trusts for its evidence, so its coverage
        // is Ville's inequality rather than an approximation, and it is valid
        // for any bounded observation whatever its shape. On this shape it
        // covers at or above nominal while landing well inside Wilson.
        assert_eq!(
            wilson_covered, trials,
            "Wilson is expected to cover every replication on this shape"
        );
        assert!(
            covered * 100 < trials * 95,
            "the normal interval undercovered when measured: {covered}/{trials}"
        );
        assert!(
            boot_covered * 100 < trials * 95,
            "the bootstrap interval undercovered when measured: {boot_covered}/{trials}"
        );
        assert!(
            betting_covered * 100 >= trials * 95,
            "the betting interval must hold nominal coverage: {betting_covered}/{trials}"
        );
        assert!(
            betting_width < wilson_width,
            "the betting interval should be narrower than Wilson: {:.4} against {:.4}",
            betting_width / trials as f64,
            wilson_width / trials as f64
        );
        assert!(
            eb_width * 2.0 < wilson_width,
            "the adaptive intervals should be far narrower: {:.4} against {:.4}",
            eb_width / trials as f64,
            wilson_width / trials as f64
        );
    }

    /// It must not narrow when the data really is maximum-variance: an
    /// interval that only ever shrinks is not adapting, it is broken.
    #[test]
    fn the_variance_adaptive_interval_stays_wide_on_coin_flips() {
        let mut rng = Rng::new(7);
        let scores: Vec<f64> = (0..120).map(|_| rng.below(2) as f64).collect();
        let (low, high) = variance_adaptive_interval(&scores);
        assert!(
            high - low > 0.15,
            "coin flips gave a {:.3}-wide interval",
            high - low
        );
        assert!(low <= 0.5 && 0.5 <= high);
    }

    #[test]
    fn resolution_counts_only_the_maps_that_broke() {
        let mostly_neutral = [0.5, 0.5, 0.5, 1.0, 0.5, 0.0];
        let directions = directional_outcomes(&mostly_neutral);
        assert_eq!(directions.neutral, 4);
        assert_eq!(resolved_maps(&directions), 2);
        assert_eq!(
            direction_sign(&directions),
            None,
            "one each way is no direction"
        );

        let decisive = [1.0, 1.0, 1.0, 0.5, 0.0];
        assert_eq!(resolved_maps(&directional_outcomes(&decisive)), 4);
        assert_eq!(direction_sign(&directional_outcomes(&decisive)), Some(true));

        let against = [0.0, 0.0, 0.5];
        assert_eq!(direction_sign(&directional_outcomes(&against)), Some(false));
        assert_eq!(direction_sign(&directional_outcomes(&[0.5, 0.5])), None);
        assert_eq!(resolved_maps(&directional_outcomes(&[])), 0);
    }

    #[test]
    fn parallel_batches_match_a_serial_run() {
        let play = |jobs: usize| {
            civvis::parallel::map(4, jobs, |index| {
                let seed = 52_000 + index as u64 / 2;
                let swap = index % 2;
                let seats: Vec<&str> = (0..2)
                    .map(|pid| {
                        if (pid + swap) % 2 == 0 {
                            "advanced"
                        } else {
                            "basic"
                        }
                    })
                    .collect();
                let mut game = Game::new(2, 20, 14, seed, 40, 0);
                let mut ais: Vec<Box<dyn Ai>> = game
                    .players
                    .iter()
                    .map(|p| {
                        let name = if p.id < 2 { seats[p.id] } else { "basic" };
                        evaluator_ai(name, seed + p.id as u64, false)
                            .expect("scripted evaluator fixture must construct")
                    })
                    .collect();
                run_traced_game(&mut game, &mut ais, 2);
                (
                    game.turn,
                    game.winner,
                    game_score(game.winner, &BTreeSet::from([0]), &BTreeSet::from([1])),
                )
            })
        };
        assert_eq!(play(1), play(4));
    }

    #[test]
    fn evaluator_factory_requires_the_strict_path_without_its_escape_flag() {
        let error = match evaluator_ai("not-a-selectable-arm", 52_100, false) {
            Ok(_) => panic!("the default evaluator factory must fail closed"),
            Err(error) => error,
        };
        assert!(matches!(error, BuiltinAiBuildError::UnknownName { .. }));
        assert!(
            evaluator_ai("not-a-selectable-arm", 52_100, true).is_ok(),
            "the diagnostic escape is the only degraded construction route"
        );
    }

    #[test]
    fn terminal_score_share_is_bounded_symmetric_and_independent_of_winner() {
        let mut game = Game::new(2, 20, 14, 71, 40, 0);
        let seats = ["challenger", "incumbent"];
        // There is already score on the board at turn 0: a start whose units
        // can see a Natural Wonder is paid Era Score for finding it, and which
        // start that is belongs to the map. Level the seats so this measures
        // the share function rather than the generator that fed it.
        for pid in 0..seats.len() {
            game.players[pid].era_score = 0;
        }
        let left = BTreeSet::from([0]);
        let right = BTreeSet::from([1]);
        let baseline = terminal_score_share(&game, &left, &right);
        assert!((baseline - 0.5).abs() < 1e-12);

        game.players[0].techs.insert(civvis::name!("writing"));
        game.winner = Some(1);
        let challenger = terminal_score_share(&game, &left, &right);
        let incumbent = terminal_score_share(&game, &right, &left);
        assert!(challenger > baseline);
        assert!((challenger + incumbent - 1.0).abs() < 1e-12);
    }

    #[test]
    fn plan_trace_counts_exposure_and_switches() {
        let mut trace = PlanTrace::default();
        for (target, strategy, rush) in [
            ("adaptive", "expansion", false),
            ("religion", "religion", true),
            ("religion", "religion", true),
            ("adaptive", "science", false),
        ] {
            trace.observe(PlanObservation {
                target,
                strategy,
                rush,
                war_enabled: false,
                war_active: false,
                context: StrategyContext {
                    at_major_war: false,
                    threatened: false,
                    city_deficit: false,
                },
                midgame: false,
            });
        }
        assert_eq!(trace.observations, 4);
        assert_eq!(trace.switches, 2);
        assert_eq!(trace.strategy_switches, 2);
        assert_eq!(trace.rush_observations, 2);
        assert!(trace.ever_rushed);
        assert_eq!(trace.targets["adaptive"], 2);
        assert_eq!(trace.targets["religion"], 2);
        assert_eq!(trace.strategy_turns["religion"], 2);
        assert_eq!(trace.dominant_target(), "adaptive");
    }

    #[test]
    fn midgame_strategy_switches_separate_visible_boundaries_from_the_residual() {
        let mut trace = PlanTrace::default();
        for (strategy, at_major_war, threatened, city_deficit) in [
            ("expansion", false, false, true),
            ("conquest", true, false, true),
            ("recovery", true, true, true),
            ("conquest", true, true, true),
        ] {
            trace.observe(PlanObservation {
                target: "adaptive",
                strategy,
                rush: false,
                war_enabled: false,
                war_active: false,
                context: StrategyContext {
                    at_major_war,
                    threatened,
                    city_deficit,
                },
                midgame: true,
            });
        }

        assert_eq!(trace.switches, 0, "the assigned target never changed");
        assert_eq!(trace.strategy_switches, 3);
        assert_eq!(trace.midgame_strategy_switches, 3);
        assert_eq!(trace.midgame_boundary_switches, 2);
        assert_eq!(trace.midgame_unanchored_switches, 1);
        assert_eq!(trace.midgame_war_boundary_switches, 1);
        assert_eq!(trace.midgame_threat_boundary_switches, 1);
        assert_eq!(trace.midgame_city_deficit_boundary_switches, 0);
        assert_eq!(trace.midgame_transitions["expansion->conquest"], 1);
        assert_eq!(trace.midgame_transitions["conquest->recovery"], 1);
        assert_eq!(trace.midgame_transitions["recovery->conquest"], 1);
    }

    #[test]
    fn empty_plan_trace_is_explicitly_unreported() {
        assert_eq!(PlanTrace::default().dominant_target(), "unreported");
    }

    #[test]
    fn traced_loop_preserves_headless_game_result() {
        let make_game = || Game::new(2, 16, 12, 9123, 30, 0);
        let mut plain = make_game();
        let mut traced = make_game();
        let mut plain_ais: Vec<Box<dyn Ai>> = (0..plain.players.len())
            .map(|pid| {
                evaluator_ai("basic", pid as u64 + 1, false)
                    .expect("scripted evaluator fixture must construct")
            })
            .collect();
        let mut traced_ais: Vec<Box<dyn Ai>> = (0..traced.players.len())
            .map(|pid| {
                evaluator_ai("basic", pid as u64 + 1, false)
                    .expect("scripted evaluator fixture must construct")
            })
            .collect();

        civvis::ai::run_game(&mut plain, &mut plain_ais);
        let traces = run_traced_game(&mut traced, &mut traced_ais, 2);

        assert_eq!(traced.winner, plain.winner);
        assert_eq!(traced.victory_type, plain.victory_type);
        assert_eq!(traced.turn, plain.turn);
        assert_eq!(traced.score(0), plain.score(0));
        assert_eq!(traced.score(1), plain.score(1));
        assert!(traces.iter().all(|trace| trace.observations > 0));
    }

    #[test]
    fn exact_sign_test_detects_replicated_map_direction() {
        let mut scores = vec![1.0; 8];
        scores.extend(vec![0.5; 16]);
        scores.push(0.0);
        let outcomes = directional_outcomes(&scores);
        assert_eq!(
            outcomes,
            DirectionalOutcomes {
                challenger_favored: 8,
                neutral: 16,
                incumbent_favored: 1,
            }
        );
        assert!((exact_sign_p(8, 1) - 0.039_062_5).abs() < 1e-12);
        assert_eq!(exact_sign_p(1, 8), exact_sign_p(8, 1));
    }

    #[test]
    fn sign_test_keeps_neutral_and_balanced_maps_inconclusive() {
        assert_eq!(directional_outcomes(&[0.5; 20]).neutral, 20);
        assert_eq!(exact_sign_p(0, 0), 1.0);
        assert_eq!(exact_sign_p(4, 4), 1.0);
    }

    #[test]
    fn draw_mixed_maps_still_have_a_direction() {
        assert_eq!(
            directional_outcomes(&[0.75, 0.25, 0.5]),
            DirectionalOutcomes {
                challenger_favored: 1,
                neutral: 1,
                incumbent_favored: 1,
            }
        );
    }
}
