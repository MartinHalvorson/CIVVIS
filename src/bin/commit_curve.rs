//! What is a victory lane worth as a function of *when* you commit to it?
//!
//! The oracle ablation (PR #366) measured that committing to Religion from
//! turn one wins 29 of 50 matched cells where the shipped adaptive agent wins
//! 14 — thirty points, against a fixed policy rather than an oracle. The churn
//! measurement (`plan_churn`, `docs/EVAL.md` 2026-07-28) found the adaptive
//! agent switching lane 14.2 times a game and spending 34.9% of it on lanes
//! that won nothing. But `refuse_unreachable_lanes` — the filter built to stop
//! exactly that — measured null at 120 maps, and its note says why: **103 of
//! 120 `advanced` wins were religious anyway.**
//!
//! Those two facts together say the agent is not picking the wrong lane. It
//! reaches the right one and reaches it *late*. That is a different defect
//! with a different fix, and this measures it directly.
//!
//! For each map one focal seat is committed to a named lane at turn `T` via
//! `AdvancedAi::retarget`, every other seat plays stock, and the same map is
//! replayed for every `T` plus an adaptive control. The engine is
//! deterministic, so a cell differs only by the treatment.
//!
//! The shape of the curve is the whole result:
//!
//! - **steeply decreasing** — commitment timing is the lever, and an honest
//!   agent that commits early captures some of the thirty points;
//! - **flat and high** — the lane matters and the timing does not, so any
//!   mechanism that ends in the right lane will do;
//! - **flat and equal to the control** — the oracle result does not reproduce
//!   here, and the difference between the harnesses has to be found before
//!   anything is built on it.
//!
//! **This calibrates itself.** `T=1` is the oracle's own condition, so it
//! should land near its 58% against a 28% control on the same profile. If it
//! does not, the rest of the curve is not to be trusted.
//!
//! ```text
//! commit_curve --lane religion --maps 40 --players 4 --width 60 --height 38 \
//!   --city-states 6 --turns 500 --seed 420000
//! ```
//!
//! Diagnostic only: it never changes a shipped decision.
use civvis::ai::{AdvancedAi, Ai, VictoryTarget, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
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

fn lane_of(name: &str) -> VictoryTarget {
    match name {
        "religion" | "religious" => VictoryTarget::Religion,
        "diplomacy" | "diplomatic" => VictoryTarget::Diplomacy,
        "culture" | "cultural" => VictoryTarget::Culture,
        "science" => VictoryTarget::Science,
        "domination" | "conquest" => VictoryTarget::Domination,
        "score" => VictoryTarget::Score,
        other => panic!("unknown lane {other}"),
    }
}

/// One (map, condition) cell.
struct Cell {
    won: bool,
    /// Did the treatment actually take? A condition that never applied would
    /// produce a null for the wrong reason, so every cell carries its own
    /// fires-check rather than trusting that `retarget` was reached.
    committed: bool,
    end_turn: u32,
    /// Cities the focal seat finished with. `assess()` sends a Religion-targeted
    /// seat that has no religion yet straight to `GrandStrategy::Religion`,
    /// bypassing the "the assigned lane can still afford to expand first" arm
    /// that every other target reaches. If that is what an early commitment
    /// costs, it shows up here and nowhere else.
    cities: usize,
}

/// Play one map with `focal` committed to `lane` at `commit`, or left adaptive
/// when `commit` is `None`.
#[allow(clippy::too_many_arguments)]
fn play(
    players: usize,
    width: i32,
    height: i32,
    seed: u64,
    turns: u32,
    city_states: usize,
    focal: usize,
    lane: VictoryTarget,
    commit: Option<u32>,
    genome: &Weights,
    may_expand: bool,
) -> Cell {
    let mut game = Game::new(players, width, height, seed, turns, city_states);
    let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, genome);
    // Only the focal seat is ever targeted, and the flag reads only inside the
    // targeted arm, so setting it here is the whole treatment.
    fleet[focal].assigned_religion_may_expand = may_expand;
    let mut committed = false;
    if commit == Some(0) {
        fleet[focal].retarget(lane);
        committed = true;
    }
    let mut end_turn = turns;
    for turn in 0..turns {
        if game.winner.is_some() {
            end_turn = turn;
            break;
        }
        if let Some(at) = commit {
            if turn == at && at > 0 {
                fleet[focal].retarget(lane);
                committed = true;
            }
        }
        for pid in 0..game.players.len() {
            if game.winner.is_some() {
                break;
            }
            fleet[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
    }
    // The target must still be held at the end; `StrategicAi` is not in play
    // here, but `adapt()` exists and a future change could clear it.
    let held = fleet[focal].victory_target() == Some(lane);
    Cell {
        won: game.winner == Some(focal),
        committed: committed && held,
        end_turn,
        cities: game.player_city_ids(focal).len(),
    }
}

/// McNemar exact (two-sided) on discordant pairs, via the binomial at p=1/2.
fn mcnemar(a: usize, b: usize) -> f64 {
    let n = a + b;
    if n == 0 {
        return 1.0;
    }
    let k = a.min(b);
    // sum_{i=0}^{k} C(n,i) / 2^n, doubled, clamped at 1.
    let mut tail = 0.0f64;
    let mut choose = 1.0f64;
    for i in 0..=k {
        if i > 0 {
            choose = choose * (n - i + 1) as f64 / i as f64;
        }
        tail += choose;
    }
    (2.0 * tail / 2f64.powi(n as i32)).min(1.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 40);
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let city_states = number(&args, "--city-states", 6);
    let seed0 = number(&args, "--seed", 420_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let lane_name = text(&args, "--lane", "religion");
    let lane = lane_of(&lane_name);
    let commits: Vec<u32> = text(&args, "--commits", "0,60,120,180")
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect();

    // Measure the agent that ships. `AdvancedAi::new()` plays
    // `Weights::default()`, which is the *fallback* the loader returns when no
    // champion is present; every deployed strategic agent resolves its genome
    // through `load_champion("evolved").unwrap_or_default()` and gets the
    // embedded gen-14 champion instead. A routing result measured on the
    // fallback would not be a result about the agent that plays.
    let genome = match text(&args, "--genome", "champion").as_str() {
        "default" => Weights::default(),
        _ => civvis::evolve::load_champion("evolved").unwrap_or_default(),
    };
    let on_champion = genome != Weights::default();

    println!(
        "commit_curve: lane {lane_name}, commits at {commits:?}, {maps} maps, \
         {players}p {width}x{height}, {city_states} city-states, {turns} turns, seed {seed0}"
    );
    println!(
        "genome: {}",
        if on_champion { "evolved champion (as shipped)" } else { "Weights::default() (the fallback)" }
    );
    println!("focal seat rotates with the map index so no seat position is privileged\n");

    let may_expand = args.iter().any(|arg| arg == "--may-expand");
    if may_expand {
        println!("treatment: assigned_religion_may_expand ON for the focal seat");
    }

    let conditions = commits.len() + 1;
    let commits_for_map = commits.clone();
    let rows = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let focal = index % players;
        let mut cells: Vec<Cell> = Vec::with_capacity(conditions);
        for at in &commits_for_map {
            cells.push(play(
                players, width, height, seed, turns, city_states, focal, lane, Some(*at),
                &genome, may_expand,
            ));
        }
        // The control is adaptive, never targeted, so the flag cannot reach it.
        cells.push(play(
            players, width, height, seed, turns, city_states, focal, lane, None, &genome, false,
        ));
        cells
    });

    let n = rows.len();
    println!("| condition | wins | share | fired | mean end turn | mean cities |");
    println!("|---|---|---|---|---|---|");
    let mut wins_by_condition: Vec<Vec<bool>> = vec![Vec::new(); conditions];
    for row in &rows {
        for (slot, cell) in row.iter().enumerate() {
            wins_by_condition[slot].push(cell.won);
        }
    }
    for slot in 0..conditions {
        let label = if slot < commits.len() {
            format!("commit at turn {}", commits[slot])
        } else {
            "adaptive (control)".to_string()
        };
        let wins = wins_by_condition[slot].iter().filter(|won| **won).count();
        let fired = rows.iter().filter(|row| row[slot].committed).count();
        let end: f64 =
            rows.iter().map(|row| row[slot].end_turn as f64).sum::<f64>() / n.max(1) as f64;
        let fired_text = if slot == commits.len() {
            "n/a".to_string()
        } else {
            format!("{fired}/{n}")
        };
        let cities: f64 =
            rows.iter().map(|row| row[slot].cities as f64).sum::<f64>() / n.max(1) as f64;
        println!(
            "| {label} | {wins}/{n} | {:.1}% | {fired_text} | {end:.0} | {cities:.2} |",
            wins as f64 * 100.0 / n.max(1) as f64
        );
    }

    // Every committed condition against the adaptive control, paired.
    let control = &wins_by_condition[commits.len()];
    println!("\nagainst the adaptive control, McNemar exact on discordant cells:");
    let mut best: Option<(u32, f64)> = None;
    for (slot, at) in commits.iter().enumerate() {
        let treated = &wins_by_condition[slot];
        let helped = treated
            .iter()
            .zip(control.iter())
            .filter(|(t, c)| **t && !**c)
            .count();
        let hurt = treated
            .iter()
            .zip(control.iter())
            .filter(|(t, c)| !**t && **c)
            .count();
        let p = mcnemar(helped, hurt);
        println!("  turn {at:>3}: {helped} helped / {hurt} hurt, p={p:.4}");
        let share = treated.iter().filter(|won| **won).count() as f64 / n.max(1) as f64;
        if best.map(|(_, b)| share > b).unwrap_or(true) {
            best = Some((*at, share));
        }
    }

    // Branch on what was measured rather than reciting a conclusion.
    let control_share = control.iter().filter(|won| **won).count() as f64 / n.max(1) as f64;
    let first = wins_by_condition[0].iter().filter(|won| **won).count() as f64 / n.max(1) as f64;
    let last_slot = commits.len() - 1;
    let last =
        wins_by_condition[last_slot].iter().filter(|won| **won).count() as f64 / n.max(1) as f64;
    println!();
    if first < control_share + 0.10 {
        println!(
            "READING: the earliest commitment ({:.1}%) is not clearly above the adaptive \
             control ({:.1}%). The oracle's thirty-point gap does NOT reproduce on this \
             harness. Find the difference between the harnesses before building anything \
             on the routing result.",
            first * 100.0,
            control_share * 100.0
        );
    } else if first - last >= 0.10 {
        println!(
            "READING: timing is the lever. Committing at turn {} wins {:.1}% and committing \
             at turn {} wins {:.1}%, against {:.1}% adaptive — the value of the lane decays \
             with the turn it is entered. An honest agent that commits early captures part \
             of this; one that merely ends up in the right lane does not.",
            commits[0],
            first * 100.0,
            commits[last_slot],
            last * 100.0,
            control_share * 100.0
        );
    } else {
        println!(
            "READING: the lane matters and the timing does not. Every commitment from turn \
             {} to turn {} scores within ten points ({:.1}% to {:.1}%) against {:.1}% \
             adaptive, so what the agent needs is to END in this lane, not to enter it \
             early. Hysteresis and early commitment are the wrong fixes; lane SELECTION is \
             the right one.",
            commits[0],
            commits[last_slot],
            first * 100.0,
            last * 100.0,
            control_share * 100.0
        );
    }
}
