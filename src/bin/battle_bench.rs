//! Matched skirmish benchmark: measure tactical play where the effect is.
//!
//! Two identical armies, the same map, the seats swapped, and a count of what
//! each agent destroyed and lost. See `src/skirmish.rs` for why whole-game win
//! rate is the wrong instrument for this subsystem and what this one does and
//! does not license.
//!
//! ```bash
//! cargo run --release --bin battle_bench -- --a advanced_joint_tactics --b advanced --games 200
//! cargo run --release --bin battle_bench -- --a advanced --b advanced --games 60   # control
//! cargo run --release --bin battle_bench -- --army warrior,warrior,archer,archer --turns 30
//! ```
//!
//! Run the control. `--a advanced --b advanced` must report a paired mean of
//! exactly zero on every seed; anything else means the swap is not cancelling
//! what it claims to cancel and no treatment number from the same harness can
//! be believed.
use civvis::ai::{run_game, Ai};
use civvis::elo::{builtin_ai, BUILTIN_AIS, EVAL_ONLY_AIS};
use civvis::game::Game;
use civvis::parallel::{default_jobs, map};
use civvis::skirmish::{matched_skirmish, MatchedSkirmish, SkirmishSetup};
use std::time::Instant;

fn number(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn known_ai(name: &str) -> bool {
    BUILTIN_AIS.contains(&name) || EVAL_ONLY_AIS.contains(&name)
}

/// Two-sided sign test: the probability of a split at least this lopsided if
/// the treatment did nothing. Ties are dropped, which is the conservative
/// convention — a harness that counts them as agreement inflates its own
/// confidence.
/// ⚠ The exact form below overflows: `2^n` is `inf` past n≈1023, the binomial
/// coefficients overflow with it, `inf / inf` is NaN, and `NaN.min(1.0)` in
/// Rust returns **1.0**. A 1122-to-317 split silently reported `p = 1.0000` —
/// a perfectly confident null on overwhelming evidence. Large n therefore uses
/// the normal approximation with a continuity correction, which is accurate to
/// far better than any decision here turns on, and the exact sum is kept for
/// the small-n case where it matters.
fn sign_test(wins: usize, losses: usize) -> f64 {
    let n = wins + losses;
    if n == 0 {
        return 1.0;
    }
    let extreme = wins.max(losses);
    if n > 1000 {
        let mean = n as f64 / 2.0;
        let sd = (n as f64 / 4.0).sqrt();
        let z = ((extreme as f64 - 0.5) - mean) / sd;
        return erfc(z / 2f64.sqrt()).clamp(0.0, 1.0);
    }
    let mut tail = 0.0f64;
    let mut coefficient = 1.0f64;
    for k in 0..=n {
        if k >= extreme || n - k >= extreme {
            tail += coefficient;
        }
        coefficient = coefficient * (n - k) as f64 / (k + 1) as f64;
    }
    (tail / 2f64.powi(n as i32)).min(1.0)
}

/// Paired t statistic and a normal-approximation two-sided p. With a hundred
/// or more pairs the normal tail is close enough to Student's, and the sign
/// test above is reported alongside precisely so a reader is not asked to
/// trust one distributional assumption on its own.
fn paired_t(differences: &[f64]) -> (f64, f64, f64, f64) {
    let n = differences.len();
    if n < 2 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let mean = differences.iter().sum::<f64>() / n as f64;
    let variance = differences
        .iter()
        .map(|d| (d - mean).powi(2))
        .sum::<f64>()
        / (n - 1) as f64;
    let stderr = (variance / n as f64).sqrt();
    if stderr <= 0.0 {
        return (mean, 0.0, f64::INFINITY, if mean == 0.0 { 1.0 } else { 0.0 });
    }
    let t = mean / stderr;
    // Two-sided normal tail via erfc.
    let p = erfc(t.abs() / 2f64.sqrt());
    (mean, stderr, t, p.clamp(0.0, 1.0))
}

/// Abramowitz & Stegun 7.1.26, good to ~1.5e-7 — far tighter than the
/// precision any decision here turns on.
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let ans = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        ans
    } else {
        2.0 - ans
    }
}

/// What a joint-planning seat costs, measured the way `turn_cost` measures a
/// searching one: **as a ratio, on interleaved runs, in the configuration that
/// would actually ship.**
///
/// A league entry is one strategy among five opponents, so the price of
/// admitting the search is `(5a + s) / 6a`, not `s / a`. `docs/EVAL.md` records
/// that measuring only the all-searching fleet once read 29x for a change that
/// costs 6.4x seated, and would have redirected the whole line of work. The
/// same trap applies here, so the default is one treated seat among five.
fn measure_cost(args: &[String], name: &str) {
    let games = number(args, "--games", 3).max(1) as usize;
    let seats = number(args, "--players", 6).max(2) as usize;
    let width = number(args, "--width", 74).max(20) as i32;
    let height = number(args, "--height", 46).max(20) as i32;
    let turns = number(args, "--turns", 100).max(10) as u32;
    let city_states = number(args, "--city-states", 9).max(0) as usize;
    let treated = number(args, "--treated", 1).max(0) as usize;
    let seed = number(args, "--seed", 8_400_000) as u64;

    println!(
        "battle_bench --cost: {games} seeds, {seats} players, {width}x{height}, \
         cap {turns} turns, {city_states} city-states, {treated} seat(s) of {name}"
    );
    println!("interleaved on the same box, so both fleets meet the same contention");

    let mut stock = (0.0f64, 0u32);
    let mut mixed = (0.0f64, 0u32);
    let mut fired = (0usize, 0usize);
    for index in 0..games {
        let gseed = seed + index as u64;
        // Interleaved: the two fleets play the same seed back to back.
        for treatment in [false, true] {
            let mut game = Game::new(seats, width, height, gseed, turns, city_states);
            let mut fleet: Vec<Box<dyn Ai>> = game
                .players
                .iter()
                .enumerate()
                .map(|(pid, player)| {
                    let major = !player.is_minor && !player.is_barbarian;
                    let pick = if treatment && major && pid < treated {
                        name
                    } else {
                        "advanced"
                    };
                    builtin_ai(pick, gseed.wrapping_add(pid as u64))
                })
                .collect();
            let started = Instant::now();
            run_game(&mut game, &mut fleet);
            let elapsed = started.elapsed().as_secs_f64();
            let played = game.turn.max(1);
            if treatment {
                mixed.0 += elapsed;
                mixed.1 += played;
                // The deployment-scale fires-check. A whole-game null means
                // something quite different depending on this number.
                for seat in fleet.iter().take(treated) {
                    if let Some((plans, decisions)) = seat.joint_tactics_census() {
                        fired.0 += plans;
                        fired.1 += decisions;
                    }
                }
            } else {
                stock.0 += elapsed;
                stock.1 += played;
            }
        }
    }

    let per_turn = |run: (f64, u32)| run.0 * 1000.0 / run.1.max(1) as f64;
    let a = per_turn(stock);
    let b = per_turn(mixed);
    println!();
    println!("all advanced          {a:.2} ms a game-turn   ({} turns)", stock.1);
    println!("{treated} seat(s) treated     {b:.2} ms a game-turn   ({} turns)", mixed.1);
    println!("ratio                 {:.2}x", b / a.max(1e-9));
    println!();
    println!(
        "fires-check: the search planned on {} turns, reaching {} unit decisions, \
         over {} treated seat-turns",
        fired.0,
        fired.1,
        mixed.1 as usize * treated
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name_a = text(&args, "--a", "advanced_joint_tactics");
    let name_b = text(&args, "--b", "advanced");
    for name in [&name_a, &name_b] {
        if !known_ai(name) {
            eprintln!("unknown agent `{name}`");
            std::process::exit(2);
        }
    }
    if args.iter().any(|arg| arg == "--cost") {
        measure_cost(&args, &name_a);
        return;
    }
    let games = number(&args, "--games", 120).max(1) as usize;
    let start_seed = number(&args, "--start-seed", 900_000) as u64;
    let jobs = number(&args, "--jobs", default_jobs() as i64).max(1) as usize;

    let mut setup = SkirmishSetup {
        turns: number(&args, "--turns", 24).max(2) as u32,
        width: number(&args, "--width", 28).max(12) as i32,
        height: number(&args, "--height", 20).max(10) as i32,
        separation: number(&args, "--separation", 6).max(2) as i32,
        ..Default::default()
    };
    let army = text(&args, "--army", "");
    if !army.is_empty() {
        setup.army = army
            .split(',')
            .map(|kind| kind.trim().to_string())
            .filter(|kind| !kind.is_empty())
            .collect();
    }

    println!(
        "battle_bench: {name_a} vs {name_b}, {games} seeds x2 seatings, \
         {} turns, army [{}], map {}x{}, separation {}",
        setup.turns,
        setup.army.join(" "),
        setup.width,
        setup.height,
        setup.separation
    );

    let results: Vec<MatchedSkirmish> = map(games, jobs, |index| {
        let seed = start_seed + index as u64;
        matched_skirmish(seed, &setup, &name_a, &name_b, &builtin_ai)
    });

    let played: Vec<&MatchedSkirmish> = results.iter().filter(|row| !row.skipped).collect();
    let skipped = results.len() - played.len();

    // The fires-check, built into the instrument. A treatment that never
    // changes the play produces a paired difference of exactly zero on every
    // seed, and a null from that is the harness saying nothing happened, not
    // the game saying it did not matter.
    let diverged = played
        .iter()
        .filter(|row| {
            row.paired_difference() != 0.0
                || row.a.damage_dealt != row.b.damage_dealt
                || row.a.kills != row.b.kills
        })
        .count();

    let differences: Vec<f64> = played.iter().map(|row| row.paired_difference()).collect();
    let wins = differences.iter().filter(|d| **d > 0.0).count();
    let losses = differences.iter().filter(|d| **d < 0.0).count();
    let ties = differences.len() - wins - losses;
    let (mean, stderr, t, p_t) = paired_t(&differences);
    let p_sign = sign_test(wins, losses);

    let sum = |pick: &dyn Fn(&MatchedSkirmish) -> f64| -> f64 {
        played.iter().map(|row| pick(row)).sum::<f64>()
    };
    let a_kills = played.iter().map(|row| row.a.kills).sum::<usize>();
    let a_losses = played.iter().map(|row| row.a.losses).sum::<usize>();
    let b_kills = played.iter().map(|row| row.b.kills).sum::<usize>();
    let b_losses = played.iter().map(|row| row.b.losses).sum::<usize>();

    println!();
    println!("seeds played           {}", played.len());
    if skipped > 0 {
        println!("seeds skipped          {skipped} (map could not seat both armies)");
    }
    println!(
        "seeds where play diverged {diverged} of {} -- the fires-check",
        played.len()
    );
    println!();
    println!("                       {name_a:>28}  {name_b:>28}");
    println!("units killed           {a_kills:>28}  {b_kills:>28}");
    println!("units lost             {a_losses:>28}  {b_losses:>28}");
    println!(
        "damage dealt           {:>28.0}  {:>28.0}",
        sum(&|row| row.a.damage_dealt),
        sum(&|row| row.b.damage_dealt)
    );
    println!(
        "material destroyed     {:>28.0}  {:>28.0}",
        sum(&|row| row.a.material_destroyed),
        sum(&|row| row.b.material_destroyed)
    );
    println!(
        "material lost          {:>28.0}  {:>28.0}",
        sum(&|row| row.a.material_lost),
        sum(&|row| row.b.material_lost)
    );
    let ratio = |kills: usize, losses: usize| {
        if losses == 0 {
            "no losses".to_string()
        } else {
            format!("{:.3}", kills as f64 / losses as f64)
        }
    };
    println!(
        "exchange ratio         {:>28}  {:>28}",
        ratio(a_kills, a_losses),
        ratio(b_kills, b_losses)
    );
    println!();
    println!("paired material swing, {name_a} less {name_b}, one number per seed:");
    println!("  mean                 {mean:+.2} +/- {stderr:.2} (standard error)");
    println!("  seeds better/worse/tied  {wins} / {losses} / {ties}");
    println!("  paired t             t = {t:.3}, p = {p_t:.4}");
    println!("  sign test            p = {p_sign:.4}");

    if diverged == 0 {
        println!();
        println!(
            "NO DIVERGENCE. The two agents played identically on every seed, so this \
             run measured nothing about either. Fix the treatment before reading the p."
        );
    }
}
