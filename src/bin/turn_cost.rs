//! What does a searching turn cost, against a scripted one?
//!
//! This is the question that decides whether any of the macro-search work can
//! ever reach a player. The deployed league roster has 61 entries and not one
//! of them searches: every seat is `AdvancedAi`, builtin or bred genome, and
//! `StrategicAi` has never played a deployed game (`docs/EVAL.md`, 2026-07-29).
//! The obvious explanation is cost — search clones the game and projects it
//! forward, a scripted turn does not — but *obvious* is not *measured*, and
//! this repository's record on unmeasured obvious things is poor.
//!
//! It matters which way it falls:
//!
//! - **If a searching turn is cheap enough to seat**, the highest-value change
//!   available is to anchor one in the league, so the rating system can see
//!   search at all. Today the self-improvement loop breeds only
//!   `StrategyKind::Advanced` genomes and cannot discover search however long it
//!   runs.
//! - **If it is not**, then effort spent making the search *stronger* is spent
//!   on an agent that cannot ship, and the honest direction is to make it
//!   *cheaper* instead. The joint axis in #589 costs 2.5× again, which would
//!   move it further from the roster rather than closer.
//!
//! ```text
//! turn_cost --games 4 --players 6 --width 74 --height 46 --turns 120
//! ```
//!
//! **Measured as a ratio, on interleaved runs, in CPU time.** This box routinely
//! sits at load 30–50 with other agents' jobs on it, so a wall-clock absolute is
//! a measurement of the neighbours. Each seed is played by both fleets back to
//! back so they meet the same contention, the report leads with the ratio, and
//! the absolutes are labelled as the loaded-box numbers they are.
//!
//! **Per game-turn, not per game.** A searching agent plays differently and its
//! games end at different lengths, so total game time confounds "thinks longer"
//! with "plays longer". The denominator is the turn count each run actually
//! reached.
use civvis::ai::{run_game, AdvancedAi};
use civvis::game::Game;
use civvis::strategic::StrategicAi;
use std::time::Instant;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One fleet's run over one seed.
struct Run {
    seconds: f64,
    turns: u32,
}

fn play_advanced(seed: u64, seats: usize, width: i32, height: i32, turns: u32, cs: usize) -> Run {
    let mut game = Game::new(seats, width, height, seed, turns, cs);
    let mut fleet = AdvancedAi::fleet(&game);
    let started = Instant::now();
    run_game(&mut game, &mut fleet);
    Run {
        seconds: started.elapsed().as_secs_f64(),
        turns: game.turn.max(1),
    }
}

/// `searching` seats play `StrategicAi`; the rest play `AdvancedAi`.
///
/// **The count is the whole point.** A fleet where every seat searches is the
/// upper bound and nothing seats that way: a league entry is *one* strategy
/// among five opponents, so the cost of admitting search is
/// `(5a + s) / 6a`, not `s / a`. Measuring only the all-searching fleet
/// answers a question nobody is asking and overstates the price about
/// fivefold.
fn play_mixed(
    seed: u64,
    seats: usize,
    width: i32,
    height: i32,
    turns: u32,
    cs: usize,
    searching: usize,
) -> Run {
    let mut game = Game::new(seats, width, height, seed, turns, cs);
    // One agent per player: city-states and barbarians take turns too, and a
    // fleet sized to the seat count indexes out of bounds the first time a
    // barbarian moves.
    let mut fleet: Vec<Box<dyn civvis::ai::Ai>> = game
        .players
        .iter()
        .enumerate()
        .map(|(pid, player)| {
            let major = !player.is_minor && !player.is_barbarian;
            if major && pid < searching {
                Box::new(StrategicAi::new()) as Box<dyn civvis::ai::Ai>
            } else {
                Box::new(AdvancedAi::new()) as Box<dyn civvis::ai::Ai>
            }
        })
        .collect();
    let started = Instant::now();
    run_game(&mut game, &mut fleet);
    Run {
        seconds: started.elapsed().as_secs_f64(),
        turns: game.turn.max(1),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 4);
    let seats = number(&args, "--players", 6);
    let width = number(&args, "--width", 74) as i32;
    let height = number(&args, "--height", 46) as i32;
    let turns = number(&args, "--turns", 120) as u32;
    let city_states = number(&args, "--city-states", 9);
    let seed = number(&args, "--seed", 105_000) as u64;

    println!(
        "turn_cost: {games} seeds, {seats} players, {width}x{height}, cap {turns} turns, \
         {city_states} city-states"
    );
    println!("interleaved, single-threaded, ratio-first -- this box is shared and absolutes drift");

    let mut advanced_total = 0.0;
    let mut strategic_total = 0.0;
    let mut advanced_turns = 0u64;
    let mut strategic_turns = 0u64;
    let mut ratios = Vec::new();
    let mut seat_ratios = Vec::new();
    let mut one_total = 0.0;
    let mut one_turns = 0u64;

    for index in 0..games {
        let this = seed + index as u64;
        // Back to back on the same seed, so both meet the same neighbours.
        let a = play_advanced(this, seats, width, height, turns, city_states);
        let one = play_mixed(this, seats, width, height, turns, city_states, 1);
        let s = play_mixed(this, seats, width, height, turns, city_states, seats);
        let a_per = a.seconds / a.turns as f64;
        let one_per = one.seconds / one.turns as f64;
        let s_per = s.seconds / s.turns as f64;
        let ratio = s_per / a_per.max(1e-9);
        let seat_ratio = one_per / a_per.max(1e-9);
        ratios.push(ratio);
        seat_ratios.push(seat_ratio);
        one_total += one.seconds;
        one_turns += one.turns as u64;
        advanced_total += a.seconds;
        strategic_total += s.seconds;
        advanced_turns += a.turns as u64;
        strategic_turns += s.turns as u64;
        println!(
            "  seed {this}: all-advanced {:.1} ms/turn | one searching seat {:.1} ms/turn \
             ({seat_ratio:.1}x) | all searching {:.1} ms/turn ({ratio:.1}x)",
            1000.0 * a_per,
            1000.0 * one_per,
            1000.0 * s_per,
        );
    }

    let advanced_per = 1000.0 * advanced_total / advanced_turns.max(1) as f64;
    let strategic_per = 1000.0 * strategic_total / strategic_turns.max(1) as f64;
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    seat_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ratios[ratios.len() / 2];
    let low = ratios[0];
    let high = ratios[ratios.len() - 1];
    let seat_median = seat_ratios[seat_ratios.len() / 2];
    let one_per = 1000.0 * one_total / one_turns.max(1) as f64;

    println!("\nadvanced:  {advanced_per:.1} ms a game-turn ({advanced_turns} turns)");
    println!("strategic: {strategic_per:.1} ms a game-turn ({strategic_turns} turns)");
    println!("one searching seat among {}: {one_per:.1} ms a game-turn", seats - 1);
    println!("ratio, all searching: median {median:.1}x, range {low:.1}x..{high:.1}x over {games} seeds");
    println!("ratio, ONE searching seat: median {seat_median:.1}x  <- the cost of seating one entry");

    // The consequence is spelled out because the whole point is to decide
    // something, and a ratio without a threshold is a number nobody acts on.
    println!(
        "\nwhat this means: {}",
        if seat_median < 3.0 {
            "cheap enough to seat -- anchoring a searching agent in the league is the \
             highest-value change available, because the loop cannot otherwise see search"
        } else if seat_median < 10.0 {
            "seatable at a real cost -- a league round would take several times as long, \
             so it is a deliberate trade rather than a free addition"
        } else {
            "too expensive to seat as it stands -- making the search stronger spends effort \
             on an agent that cannot ship, and the direction is to make it cheaper instead"
        }
    );
    println!(
        "note: the joint axis (#589) costs about 2.5x again on top of whatever this says."
    );
}
