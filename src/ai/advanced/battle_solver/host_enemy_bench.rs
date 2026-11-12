//! The solver with the host-fitted enemy against the solver with the stock
//! enemy, on `battle_bench` skirmishes and `doctrine_arena` positions, at
//! horizon 2 and 4.
//!
//! ⚠ A no-regression read, not the evidence for the gene. The opponent on
//! both benches is this repository's own AI (the live tactical stack), not
//! the host, so the fitted model is if anything a worse forecast of it than
//! the stock guess. What `solver-plays-the-host` rests on is the held-out
//! likelihood of the host's recorded moves in `docs/HOST_ENEMY_MODEL.md`.
//!
//! `cargo test --lib host_enemy_bench -- --ignored --nocapture`, with
//! `CIVVIS_HOST_BENCH_SEEDS` for the seeds per row.

use std::time::Instant;

use super::TEST_HORIZON;
use crate::doctrine::{matched_position, position};
use crate::elo::seat_ai;
use crate::parallel::{default_jobs, map};
use crate::skirmish::{matched_skirmish, SkirmishSetup};

/// The tactical genes the live seat runs, with the solver: the seat the
/// solver's own bench measured (`docs/BATTLE_SOLVER.md`).
const STOCK: &str = "advanced+battle-planner-2+slots-survive-the-roll+safest-stand+shared-danger+ranged-before-melee+ranged-finish-on-the-mean+ranged-hp-reserve+anvil-spares-the-one-blow+battle-solver";
/// The same seat with the fitted enemy.
const HOST: &str = "advanced+battle-planner-2+slots-survive-the-roll+safest-stand+shared-danger+ranged-before-melee+ranged-finish-on-the-mean+ranged-hp-reserve+anvil-spares-the-one-blow+battle-solver+solver-plays-the-host";

/// A block of seeds fresh to the solver's tuning (9,700,000).
const SEED: u64 = 9_800_000;

const ARMIES: [(&str, &[&str]); 4] = [
    (
        "combined arms",
        &[
            "warrior", "warrior", "spearman", "archer", "archer", "horseman",
        ],
    ),
    (
        "ranged heavy",
        &[
            "warrior", "spearman", "archer", "archer", "archer", "archer",
        ],
    ),
    (
        "melee only",
        &[
            "warrior",
            "warrior",
            "warrior",
            "spearman",
            "spearman",
            "swordsman",
        ],
    ),
    (
        "medieval mounted",
        &[
            "pikeman",
            "swordsman",
            "crossbowman",
            "crossbowman",
            "knight",
            "knight",
        ],
    ),
];

const POSITIONS: [&str; 5] = [
    "central_position",
    "the_defile",
    "the_ridge",
    "hammer_and_anvil",
    "the_relief",
];

/// One row: the paired material swing of `a` less `b`, its standard error,
/// seeds better / worse / tied, kills and losses of each side, and seeds
/// where play diverged.
struct Row {
    mean: f64,
    stderr: f64,
    better: usize,
    worse: usize,
    tied: usize,
    a: (usize, usize),
    b: (usize, usize),
    diverged: usize,
    seeds: usize,
}

fn row(differences: &[f64], a: (usize, usize), b: (usize, usize), diverged: usize) -> Row {
    let n = differences.len().max(1) as f64;
    let mean = differences.iter().sum::<f64>() / n;
    let variance = differences
        .iter()
        .map(|d| (d - mean) * (d - mean))
        .sum::<f64>()
        / (n - 1.0).max(1.0);
    Row {
        mean,
        stderr: (variance / n).sqrt(),
        better: differences.iter().filter(|d| **d > 0.0).count(),
        worse: differences.iter().filter(|d| **d < 0.0).count(),
        tied: differences.iter().filter(|d| **d == 0.0).count(),
        a,
        b,
        diverged,
        seeds: differences.len(),
    }
}

fn skirmish_row(army: &[&str], seeds: usize, a: &str, b: &str) -> Row {
    let setup = SkirmishSetup {
        army: army.iter().map(|kind| kind.to_string()).collect(),
        ..Default::default()
    };
    let results = map(seeds, default_jobs(), |k| {
        matched_skirmish(SEED + k as u64, &setup, a, b, &seat_ai)
    });
    let played: Vec<_> = results.iter().filter(|r| !r.skipped).collect();
    let differences: Vec<f64> = played.iter().map(|r| r.paired_difference()).collect();
    let diverged = played
        .iter()
        .filter(|r| {
            r.paired_difference() != 0.0
                || r.a.damage_dealt != r.b.damage_dealt
                || r.a.kills != r.b.kills
        })
        .count();
    row(
        &differences,
        (
            played.iter().map(|r| r.a.kills).sum(),
            played.iter().map(|r| r.a.losses).sum(),
        ),
        (
            played.iter().map(|r| r.b.kills).sum(),
            played.iter().map(|r| r.b.losses).sum(),
        ),
        diverged,
    )
}

fn position_row(id: &str, seeds: usize, a: &str, b: &str) -> Row {
    let spec = position(id).unwrap_or_else(|| panic!("no doctrine position `{id}`"));
    let results = map(seeds, default_jobs(), |k| {
        matched_position(spec, SEED + k as u64, a, b, &seat_ai)
    });
    let played: Vec<_> = results.iter().filter(|r| !r.skipped).collect();
    let differences: Vec<f64> = played.iter().map(|r| r.paired_difference()).collect();
    let diverged = played
        .iter()
        .filter(|r| r.paired_difference() != 0.0 || r.a.kills != r.b.kills)
        .count();
    row(
        &differences,
        (
            played.iter().map(|r| r.a.kills).sum(),
            played.iter().map(|r| r.a.losses).sum(),
        ),
        (
            played.iter().map(|r| r.b.kills).sum(),
            played.iter().map(|r| r.b.losses).sum(),
        ),
        diverged,
    )
}

fn line(name: &str, r: &Row, started: Instant) -> String {
    format!(
        "{name:<34} {:+9.2} +/- {:7.2}  {:>3}/{:>3}/{:>3}  kills/losses fitted {:>4}/{:<4} stock {:>4}/{:<4}  diverged {:>3}/{:<3}  {:>5.0}s",
        r.mean,
        r.stderr,
        r.better,
        r.worse,
        r.tied,
        r.a.0,
        r.a.1,
        r.b.0,
        r.b.1,
        r.diverged,
        r.seeds,
        started.elapsed().as_secs_f64()
    )
}

fn seeds(default: usize) -> usize {
    std::env::var("CIVVIS_HOST_BENCH_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The control, both ways: the same seat on both sides reads exactly zero on
/// every seed with the gene on and with it off. Anything else means the
/// pairing does not cancel and no number from the bench can be read.
#[test]
fn the_bench_control_reads_zero_with_the_gene_on_and_off() {
    for seat in [HOST, STOCK] {
        let r = skirmish_row(ARMIES[0].1, 2, seat, seat);
        assert_eq!(r.seeds, 2, "{seat}");
        assert_eq!((r.mean, r.diverged), (0.0, 0), "{seat}");
    }
}

/// Fitted-enemy solver (seat A) against stock-enemy solver (seat B).
#[test]
fn host_enemy_bench() {
    // WIP: CI evidence run; the report rides the panic into the log.
    let skirmish_seeds = seeds(8);
    let position_seeds = (skirmish_seeds / 2).max(2);
    let mut report = vec![format!(
        "solver-plays-the-host bench: A = live tactical stack + battle-solver + solver-plays-the-host, \
         B = the same without solver-plays-the-host; paired material swing A less B, mean +/- se, \
         seeds better/worse/tied; {skirmish_seeds} skirmish seeds, {position_seeds} position seeds, \
         seed block {SEED}, {} threads",
        default_jobs()
    )];
    let started = Instant::now();
    for horizon in [2u32, 4] {
        TEST_HORIZON.store(horizon, std::sync::atomic::Ordering::Relaxed);
        for seat in [HOST, STOCK] {
            let r = skirmish_row(ARMIES[0].1, 2, seat, seat);
            report.push(line(
                &format!(
                    "h{horizon} control {}",
                    if seat == HOST { "on/on" } else { "off/off" }
                ),
                &r,
                started,
            ));
            assert_eq!((r.mean, r.diverged), (0.0, 0), "control h{horizon} {seat}");
        }
        for (name, army) in ARMIES {
            let r = skirmish_row(army, skirmish_seeds, HOST, STOCK);
            report.push(line(
                &format!("h{horizon} battle_bench {name}"),
                &r,
                started,
            ));
        }
        for id in POSITIONS {
            let r = position_row(id, position_seeds, HOST, STOCK);
            report.push(line(&format!("h{horizon} doctrine {id}"), &r, started));
        }
    }
    TEST_HORIZON.store(0, std::sync::atomic::Ordering::Relaxed);
    panic!("HOST-BENCH-REPORT\n{}", report.join("\n"));
}
