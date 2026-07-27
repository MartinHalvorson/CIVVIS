//! Paired fires-check for public-state rival modeling in Strategic rollouts.
//!
//! A rollout currently reconstructs every opponent as a fresh adaptive
//! `AdvancedAi`. The treatment asks a narrower question: when public
//! victory-screen progress identifies a rival's lane with a useful margin,
//! does keeping that lane fixed through the projection change what the macro
//! search sees? This probe answers before an expensive strength evaluation.
//!
//! Both arms read the same warmed game and the same searching agent. The
//! baseline uses stock adaptive rivals; the treatment changes only
//! `model_rival_lanes`. A treatment must clear both parts of the fires-check:
//! infer non-adaptive rivals at a substantial share of rollout roots, and
//! change branch values or lane decisions on those exact positions. Passing
//! earns a fresh mirrored `ai_eval`; it is not evidence of strength.
//!
//! ```text
//! cargo run --profile ci --bin rival_probe -- \
//!   --players 4 --maps 24 --warmup 60 --seed 93000 --jobs 8
//! ```
use civvis::ai::{AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::strategic::{ReviewPath, StrategicAi};

const DECISION_MARGIN: f64 = 0.01;

struct Reading {
    inferred: [usize; 6],
    rivals: usize,
    changed_branches: usize,
    branches: usize,
    max_delta: f64,
    total_delta: f64,
    base_spread: f64,
    treated_spread: f64,
    base_target: Option<VictoryTarget>,
    treated_target: Option<VictoryTarget>,
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn lane_index(target: VictoryTarget) -> usize {
    match target {
        VictoryTarget::Science => 0,
        VictoryTarget::Culture => 1,
        VictoryTarget::Religion => 2,
        VictoryTarget::Diplomacy => 3,
        VictoryTarget::Domination => 4,
        VictoryTarget::Score => 5,
    }
}

fn lane_name(target: Option<VictoryTarget>) -> &'static str {
    target.map_or("adaptive", VictoryTarget::as_str)
}

fn spread(values: &[(f64, Option<VictoryTarget>)]) -> f64 {
    let low = values
        .iter()
        .map(|(value, _)| *value)
        .fold(f64::INFINITY, f64::min);
    let high = values
        .iter()
        .map(|(value, _)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    high - low
}

/// Reproduce Strategic's private commitment rule from its public branch
/// values. The adaptive branch wins unless the best named lane clears it by
/// the production margin.
fn selected_target(values: &[(f64, Option<VictoryTarget>)]) -> Option<VictoryTarget> {
    let adaptive = values
        .iter()
        .find_map(|(value, target)| target.is_none().then_some(*value))?;
    values
        .iter()
        .filter_map(|(value, target)| target.map(|target| (*value, target)))
        .max_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|(value, _)| *value > adaptive + DECISION_MARGIN)
        .map(|(_, target)| target)
}

fn sign_p(up: usize, down: usize) -> f64 {
    let n = up + down;
    if n == 0 {
        return 1.0;
    }
    let k = up.min(down);
    let mut tail = 0.0;
    for i in 0..=k {
        let mut term = 0.0;
        for j in 0..i {
            term += ((n - j) as f64).ln() - ((j + 1) as f64).ln();
        }
        tail += (term - n as f64 * std::f64::consts::LN_2).exp();
    }
    (2.0 * tail).min(1.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let warmup = number(&args, "--warmup", 60) as u32;
    let seed0 = number(&args, "--seed", 93_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let weights = civvis::evolve::load_champion("evolved").unwrap_or_default();

    let results = parallel::map(maps, jobs, |index| {
        let mut game = Game::new(players, width, height, seed0 + index as u64, turns, 0);
        let mut agent = StrategicAi::with_weights(weights.clone());
        let mut opponents = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    agent.take_turn(&mut game, pid);
                } else {
                    opponents[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() || agent.review_detailed(&game, 0).1 != ReviewPath::Rollouts {
            return None;
        }

        let mut inferred = [0usize; 6];
        let mut rivals = 0;
        for rival in game.players.iter().filter(|player| {
            player.id != 0 && player.alive && !player.is_minor && !player.is_barbarian
        }) {
            rivals += 1;
            if let Some(target) = agent.inferred_rival_target(&game, rival.id) {
                inferred[lane_index(target)] += 1;
            }
        }

        let base = agent.lane_values(&game, 0);
        agent.model_rival_lanes = true;
        let treated = agent.lane_values(&game, 0);
        assert_eq!(
            base.iter().map(|(_, target)| target).collect::<Vec<_>>(),
            treated.iter().map(|(_, target)| target).collect::<Vec<_>>()
        );
        let deltas: Vec<f64> = base
            .iter()
            .zip(&treated)
            .map(|((left, _), (right, _))| (right - left).abs())
            .collect();
        Some(Reading {
            inferred,
            rivals,
            changed_branches: deltas.iter().filter(|delta| **delta > 1e-12).count(),
            branches: deltas.len(),
            max_delta: deltas.iter().copied().fold(0.0, f64::max),
            total_delta: deltas.iter().sum(),
            base_spread: spread(&base),
            treated_spread: spread(&treated),
            base_target: selected_target(&base),
            treated_target: selected_target(&treated),
        })
    });

    let readings: Vec<Reading> = results.into_iter().flatten().collect();
    if readings.is_empty() {
        println!("no warmed position reached the rollout path in {maps} maps");
        std::process::exit(1);
    }

    let rivals: usize = readings.iter().map(|reading| reading.rivals).sum();
    let mut inferred = [0usize; 6];
    for reading in &readings {
        for (total, count) in inferred.iter_mut().zip(reading.inferred) {
            *total += count;
        }
    }
    let inferred_total: usize = inferred.iter().sum();
    let changed_positions = readings
        .iter()
        .filter(|reading| reading.changed_branches > 0)
        .count();
    let changed_branches: usize = readings
        .iter()
        .map(|reading| reading.changed_branches)
        .sum();
    let branches: usize = readings.iter().map(|reading| reading.branches).sum();
    let total_delta: f64 = readings.iter().map(|reading| reading.total_delta).sum();
    let max_delta = readings
        .iter()
        .map(|reading| reading.max_delta)
        .fold(0.0, f64::max);
    let flips: Vec<&Reading> = readings
        .iter()
        .filter(|reading| reading.base_target != reading.treated_target)
        .collect();
    let spread_up = readings
        .iter()
        .filter(|reading| reading.treated_spread > reading.base_spread)
        .count();
    let spread_down = readings
        .iter()
        .filter(|reading| reading.treated_spread < reading.base_spread)
        .count();
    let spread_same = readings.len() - spread_up - spread_down;

    println!(
        "rival probe: {} of {maps} warmed positions reached rollouts \
         ({players}p {width}x{height}, warmup {warmup}, seeds {seed0}..)",
        readings.len()
    );
    println!(
        "inference: {inferred_total}/{rivals} rival seats targeted ({:.1}%)",
        100.0 * inferred_total as f64 / rivals.max(1) as f64
    );
    for target in VictoryTarget::ALL {
        let count = inferred[lane_index(target)];
        if count > 0 {
            println!("  {:<12} {count}", target.as_str());
        }
    }
    println!(
        "branch effects: {changed_positions}/{} positions, {changed_branches}/{branches} values; \
         mean |delta| {:.5}, max |delta| {:.5}",
        readings.len(),
        total_delta / branches.max(1) as f64,
        max_delta
    );
    println!(
        "spread: higher {spread_up}, lower {spread_down}, identical {spread_same}; sign p={:.4}",
        sign_p(spread_up, spread_down)
    );
    println!(
        "decision flips: {}/{} positions",
        flips.len(),
        readings.len()
    );
    for reading in flips.iter().take(10) {
        println!(
            "  {:<12} -> {}",
            lane_name(reading.base_target),
            lane_name(reading.treated_target)
        );
    }

    if inferred_total * 4 < rivals {
        println!(
            "\nFAIL — fewer than 25% of rival seats were legible, so this is not a \
             substantial opponent model on the sampled distribution."
        );
        std::process::exit(3);
    }
    if changed_positions == 0 {
        println!(
            "\nINERT — inferred lanes changed no branch value. A strength evaluation \
             would measure nothing."
        );
        std::process::exit(3);
    }
    println!(
        "\nFIRES — public rival lanes are common and move the exact values Strategic \
         consumes. This earns a fresh mirrored evaluation; it does not establish strength."
    );
}
