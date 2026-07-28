//! How often does this agent change its mind about what it is playing for?
//!
//! The oracle ablation (`docs/EVAL.md`, 2026-07-26) found that capability is
//! not what limits this agent and routing is: committing to Religion from turn
//! one wins 58% of 50 matched cells where the shipped adaptive agent wins 28%,
//! McNemar exact p=0.0000. That is a thirty-point gap against a *fixed* policy,
//! not against an oracle, so it is not explained away by hindsight.
//!
//! A fixed policy can beat an adaptive one for exactly two reasons. Either the
//! adaptive agent picks the wrong lane, or it picks the right lane and does not
//! stay in it long enough for the investment to pay. Civ lanes compound — a
//! Holy Site bought on turn 40 pays a Prophet on turn 70 and apostles for the
//! next two hundred — so an agent that re-derives its lane from the current
//! board, with no memory of what it has already bought, will keep abandoning
//! half-finished programmes for whichever lane looks cheapest right now.
//!
//! `AdvancedAi::plan_stale` re-assesses every 5 turns, and `assess()` computes
//! the strategy from scratch: nothing in it reads the previous plan. So a
//! 500-turn game contains up to a hundred independent re-decisions with no
//! hysteresis at any of them. Whether that *matters* is an empirical question,
//! and this answers it.
//!
//! Per major seat, per game, it records the grand strategy every turn and
//! reports:
//!
//! - **switches**: how many times the strategy changed, and per 100 turns;
//! - **run length**: mean and longest unbroken commitment to one strategy;
//! - **distinct**: how many of the seven the seat visited at all;
//! - **fragmentation**: the share of runs that lasted under 10 turns, which is
//!   under one Holy Site and under half a settler;
//! - the share of turns spent in each strategy, and — for seats that won —
//!   how much of the game was spent in the lane the seat actually won by.
//!
//! The reading that clears the current design is long runs and few switches:
//! the agent commits, and the 5-turn cadence is just re-confirming a settled
//! choice. The reading that condemns it is a mean run length short against the
//! payback period of the things a lane asks you to buy.
//!
//! ```text
//! plan_churn --players 4 --maps 12 --turns 500
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// A run shorter than this buys nothing a lane asks for. A Holy Site plus the
/// Prophet it exists to produce is far longer; a settler walking to a site is
/// about this. It is a deliberately generous floor.
const SHORT_RUN: usize = 10;

/// One seat's history of grand strategies, one entry per turn played.
struct Seat {
    history: Vec<&'static str>,
    won: bool,
}

impl Seat {
    /// Consecutive equal labels, collapsed to (label, length).
    fn runs(&self) -> Vec<(&'static str, usize)> {
        let mut runs: Vec<(&'static str, usize)> = Vec::new();
        for label in &self.history {
            match runs.last_mut() {
                Some((current, count)) if current == label => *count += 1,
                _ => runs.push((label, 1)),
            }
        }
        runs
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 12);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 900_000) as u64;
    // City-states are not decoration for this measurement: the diplomatic lane
    // runs on them, so a run without any cannot be read as evidence that the
    // agent declines diplomacy.
    let city_states = number(&args, "--city-states", 0);
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "plan_churn: {maps} maps, {players}p {width}x{height}, {turns} turns, \
         {city_states} city-states, seed {seed0}, short run < {SHORT_RUN} turns"
    );

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, city_states);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();
        let mut seats: Vec<Seat> = majors
            .iter()
            .map(|_| Seat { history: Vec::new(), won: false })
            .collect();

        for _ in 0..turns {
            if game.winner.is_some() {
                break;
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
            // Read the plan after the whole round so every seat is sampled at
            // the same point in the turn order.
            for (slot, pid) in majors.iter().enumerate() {
                if let Some(label) = fleet[*pid].strategy_label() {
                    seats[slot].history.push(label);
                }
            }
        }
        if let Some(winner) = game.winner {
            if let Some(slot) = majors.iter().position(|pid| *pid == winner) {
                seats[slot].won = true;
            }
        }
        seats
    });

    let seats: Vec<Seat> = per_map.into_iter().flatten().collect();
    let scored: Vec<&Seat> = seats.iter().filter(|seat| !seat.history.is_empty()).collect();
    if scored.is_empty() {
        println!("no seat ever produced a plan; nothing to report");
        return;
    }

    let mut switches_total = 0usize;
    let mut turns_total = 0usize;
    let mut runs_total = 0usize;
    let mut short_runs = 0usize;
    let mut longest_sum = 0usize;
    let mut distinct_sum = 0usize;
    let mut shares: std::collections::BTreeMap<&'static str, usize> = Default::default();
    // Of the turns a winning seat played, how many were spent in the strategy
    // it held at the end — the closest observable proxy for "the lane it won
    // by" without reaching into victory bookkeeping.
    let mut winners = 0usize;
    let mut winner_final_share = 0.0f64;

    for seat in &scored {
        let runs = seat.runs();
        switches_total += runs.len().saturating_sub(1);
        turns_total += seat.history.len();
        runs_total += runs.len();
        short_runs += runs.iter().filter(|(_, len)| *len < SHORT_RUN).count();
        longest_sum += runs.iter().map(|(_, len)| *len).max().unwrap_or(0);
        let mut seen: std::collections::BTreeSet<&'static str> = Default::default();
        for label in &seat.history {
            seen.insert(label);
            *shares.entry(label).or_default() += 1;
        }
        distinct_sum += seen.len();
        if seat.won {
            winners += 1;
            let final_label = seat.history[seat.history.len() - 1];
            let held = seat.history.iter().filter(|label| **label == final_label).count();
            winner_final_share += held as f64 / seat.history.len() as f64;
        }
    }

    let n = scored.len() as f64;
    let mean_run = turns_total as f64 / runs_total.max(1) as f64;
    let per_100 = switches_total as f64 * 100.0 / turns_total.max(1) as f64;
    let short_share = short_runs as f64 * 100.0 / runs_total.max(1) as f64;

    println!("\nseats scored          {}", scored.len());
    println!("turns per seat        {:.1}", turns_total as f64 / n);
    println!("distinct strategies   {:.2} of 7", distinct_sum as f64 / n);
    println!("switches per seat     {:.1}", switches_total as f64 / n);
    println!("switches per 100t     {per_100:.2}");
    println!("mean run length       {mean_run:.1} turns");
    println!("longest run           {:.1} turns", longest_sum as f64 / n);
    println!("runs under {SHORT_RUN} turns    {short_share:.1}% of all runs");
    if winners > 0 {
        println!(
            "winning seats         {winners}, holding their final strategy {:.1}% of the game",
            winner_final_share * 100.0 / winners as f64
        );
    }

    println!("\nshare of turns by strategy");
    let mut ranked: Vec<(&&'static str, &usize)> = shares.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1));
    for (label, count) in ranked {
        println!("  {label:<10} {:5.1}%", *count as f64 * 100.0 / turns_total as f64);
    }

    // Branch on what was measured. The point of the run is to decide between
    // two live readings, so state which one the numbers support rather than
    // reciting a conclusion written before the data existed.
    println!();
    if mean_run >= 60.0 && per_100 < 2.0 {
        println!(
            "READING: the agent commits. Mean run {mean_run:.1} turns against a 5-turn \
             re-assessment cadence means the periodic re-derivation mostly re-confirms a \
             settled choice, and lane churn is NOT what separates it from a fixed policy."
        );
    } else if mean_run < 25.0 || per_100 >= 4.0 {
        println!(
            "READING: the agent churns. Mean run {mean_run:.1} turns and {per_100:.2} switches \
             per 100 turns, with {short_share:.1}% of runs under {SHORT_RUN} turns — shorter \
             than the payback period of the districts and units a lane asks it to buy. \
             Hysteresis in the lane decision is a live hypothesis."
        );
    } else {
        println!(
            "READING: intermediate. Mean run {mean_run:.1} turns, {per_100:.2} switches per 100 \
             turns. Neither reading is clean; compare the run length against the specific \
             payback period of the lane the seat was in before concluding."
        );
    }
}
