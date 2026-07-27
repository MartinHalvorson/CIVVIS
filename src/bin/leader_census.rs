//! When somebody is about to win, does anybody notice in time?
//!
//! The AI already has a denial layer: `victory_denial` names the rival closest
//! to a win and hands back a counter-strategy, and `urgent_victory_threat`
//! lets a short clock skip the ordinary war-readiness checks. What nothing has
//! measured is whether that alarm ever rings *before the game is already over*.
//!
//! An alarm that fires the turn before the win is not a defence, it is a
//! commentary track. So this census reads, on every turn of a full game and
//! for every living major:
//!
//! - **the honest meter** — `Game::victory_threat`, the same arithmetic the
//!   victory screen shows;
//! - **the meter the AI actually gates on** — `AdvancedAi::rival_pressure`,
//!   a second implementation that nothing has ever compared against the first;
//! - **whether anybody named them** — `denial_target` across every other
//!   living major, so a firing is counted where the decision is made;
//! - **whether anybody moved** — how many majors are at war with them.
//!
//! The readings that matter, all reported against the *eventual winner*:
//!
//! - **lead time**: turns between the first alarm and the win. Near zero means
//!   the layer cannot work no matter how good the response is.
//! - **blind wins**: the share of wins nobody ever raised an alarm about.
//! - **meter lag**: how many turns later the AI's meter crosses its bar than
//!   the honest meter crosses the same number. A positive median means the AI
//!   is reading a dimmer instrument than the one on screen.
//! - **response**: majors at war with the leader before the first alarm versus
//!   after it. Flat means the alarm changes no behaviour.
//!
//! ```text
//! leader_census --players 4 --maps 24 --turns 400 --seed 900000
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, GrandStrategy, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use std::collections::BTreeMap;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// The bar `victory_denial` uses for every non-religious race.
const DENIAL_BAR: i32 = 78;
/// The honest meter compared at the same number, so "lag" is a like-for-like
/// reading of two instruments and not of two thresholds.
const HONEST_BAR: f64 = 78.0;

#[derive(Clone, Default)]
struct Track {
    /// First turn the AI's own meter put this empire at or past the bar.
    first_seen: Option<u32>,
    /// First turn the urgent predicate was true for this empire.
    first_urgent: Option<u32>,
    /// First turn `Game::victory_threat` put it past the same number.
    first_honest: Option<u32>,
    /// First turn any other living major named it in `denial_target`.
    first_named: Option<u32>,
    /// Distinct rivals that ever named it.
    namers: usize,
    /// Turns spent at war with at least one major, split at `first_named`.
    war_turns_before: u32,
    war_turns_after: u32,
    turns_before: u32,
    turns_after: u32,
    /// Lane the AI's meter attributed to it on the last turn it was read.
    last_lane: Option<GrandStrategy>,
    /// Highest reading each instrument ever gave for this empire.
    peak_seen: i32,
    peak_honest: f64,
    /// Signed gap between the two instruments, sampled every turn the honest
    /// meter had anything to say. A negative mean is the AI reading *lower*
    /// than the victory screen — the disagreement PR #291 warned about.
    gap_total: f64,
    gap_samples: u32,
}

struct MapReading {
    winner: Option<usize>,
    victory_type: String,
    end_turn: u32,
    tracks: BTreeMap<usize, Track>,
    /// Player-turns where a denial fired at all, and where it named the
    /// empire that went on to win.
    denials: u64,
    denials_on_winner: u64,
    observations: u64,
}

fn note(slot: &mut Option<u32>, turn: u32) {
    if slot.is_none() {
        *slot = Some(turn);
    }
}

/// Median of a set of readings, or `None` when empty.
fn median(values: &mut Vec<i64>) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 400) as u32;
    let seed0 = number(&args, "--seed", 900_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "leader_census: {maps} maps, {players}p {width}x{height}, {turns} turns, seed {seed0}, \
         denial bar {DENIAL_BAR}"
    );

    let readings = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        // `rival_pressure` reads nothing from the planner it is asked of, so
        // one probe answers for every seat and the reading cannot drift with
        // whichever empire happens to be looking.
        let probe = AdvancedAi::new();
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();
        let mut tracks: BTreeMap<usize, Track> =
            majors.iter().map(|pid| (*pid, Track::default())).collect();
        let mut denials = 0_u64;
        let mut named_by_turn: Vec<(u32, usize)> = Vec::new();
        let mut observations = 0_u64;
        let mut end_turn = turns;

        for turn in 0..turns {
            if game.winner.is_some() {
                end_turn = turn;
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
            end_turn = turn;

            // Who is close, on each instrument.
            for target in majors.iter().copied() {
                if !game.players[target].alive {
                    continue;
                }
                let (lane, seen) = probe.rival_pressure(&game, target);
                let honest = game.victory_threat(target);
                let track = tracks.get_mut(&target).expect("major tracked");
                track.last_lane = Some(lane);
                track.peak_seen = track.peak_seen.max(seen);
                track.peak_honest = track.peak_honest.max(honest);
                if honest > 0.0 {
                    track.gap_total += seen as f64 - honest;
                    track.gap_samples += 1;
                }
                if seen >= DENIAL_BAR {
                    note(&mut track.first_seen, turn);
                }
                if honest >= HONEST_BAR {
                    note(&mut track.first_honest, turn);
                }
                if probe.denial_is_urgent(&game, target) {
                    note(&mut track.first_urgent, turn);
                }
            }

            // Who would move, read where the decision is actually made.
            for observer in majors.iter().copied() {
                if !game.players[observer].alive {
                    continue;
                }
                observations += 1;
                if let Some((rival, _counter)) = fleet[observer].denial_target(&game, observer) {
                    denials += 1;
                    named_by_turn.push((turn, rival));
                    let track = tracks.get_mut(&rival).expect("major tracked");
                    note(&mut track.first_named, turn);
                }
            }

            // Did anybody actually go to war with them, and when relative to
            // the first alarm.
            for target in majors.iter().copied() {
                if !game.players[target].alive {
                    continue;
                }
                let at_war = majors
                    .iter()
                    .any(|other| *other != target && game.is_at_war(target, *other));
                let track = tracks.get_mut(&target).expect("major tracked");
                let after = track.first_named.is_some_and(|first| turn >= first);
                if after {
                    track.turns_after += 1;
                    if at_war {
                        track.war_turns_after += 1;
                    }
                } else {
                    track.turns_before += 1;
                    if at_war {
                        track.war_turns_before += 1;
                    }
                }
            }
        }

        for target in majors.iter().copied() {
            let namers = named_by_turn
                .iter()
                .filter(|(_, rival)| *rival == target)
                .count();
            if let Some(track) = tracks.get_mut(&target) {
                track.namers = namers;
            }
        }
        let winner = game.winner;
        let denials_on_winner = winner.map_or(0, |w| {
            named_by_turn.iter().filter(|(_, rival)| *rival == w).count() as u64
        });

        MapReading {
            winner,
            victory_type: game.victory_type.clone().unwrap_or_else(|| "none".into()),
            end_turn,
            tracks,
            denials,
            denials_on_winner,
            observations,
        }
    });

    let decided: Vec<&MapReading> = readings.iter().filter(|r| r.winner.is_some()).collect();
    let denials: u64 = readings.iter().map(|r| r.denials).sum();
    let observations: u64 = readings.iter().map(|r| r.observations).sum();
    let on_winner: u64 = readings.iter().map(|r| r.denials_on_winner).sum();

    println!(
        "\ngames: {} of {maps} decided ({:.0}%)",
        decided.len(),
        100.0 * decided.len() as f64 / maps.max(1) as f64
    );
    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    for reading in &readings {
        *by_type.entry(reading.victory_type.as_str()).or_default() += 1;
    }
    for (kind, count) in &by_type {
        println!("  {kind:<12} {count}");
    }

    println!(
        "\ndenial fires on {denials} of {observations} player-turns ({:.1}%); \
         {on_winner} of those name the eventual winner ({:.1}% of firings)",
        100.0 * denials as f64 / observations.max(1) as f64,
        100.0 * on_winner as f64 / denials.max(1) as f64
    );

    // Everything below reads the eventual winner only: the empire the rest of
    // the table had every reason to stop.
    let mut lead_named: Vec<i64> = Vec::new();
    let mut lead_seen: Vec<i64> = Vec::new();
    let mut lead_honest: Vec<i64> = Vec::new();
    let mut lead_urgent: Vec<i64> = Vec::new();
    let mut meter_lag: Vec<i64> = Vec::new();
    let mut blind = 0_usize;
    let mut never_seen = 0_usize;
    let mut war_before = (0_u32, 0_u32);
    let mut war_after = (0_u32, 0_u32);
    let mut winning_lane: BTreeMap<&str, usize> = BTreeMap::new();

    for reading in &decided {
        let winner = reading.winner.expect("decided");
        let Some(track) = reading.tracks.get(&winner) else {
            continue;
        };
        let end = reading.end_turn as i64;
        match track.first_named {
            Some(first) => lead_named.push(end - first as i64),
            None => blind += 1,
        }
        match track.first_seen {
            Some(first) => lead_seen.push(end - first as i64),
            None => never_seen += 1,
        }
        if let Some(first) = track.first_honest {
            lead_honest.push(end - first as i64);
        }
        if let Some(first) = track.first_urgent {
            lead_urgent.push(end - first as i64);
        }
        if let (Some(seen), Some(honest)) = (track.first_seen, track.first_honest) {
            meter_lag.push(seen as i64 - honest as i64);
        }
        war_before.0 += track.war_turns_before;
        war_before.1 += track.turns_before;
        war_after.0 += track.war_turns_after;
        war_after.1 += track.turns_after;
        if let Some(lane) = track.last_lane {
            *winning_lane.entry(lane.as_str()).or_default() += 1;
        }
    }

    println!("\nthe eventual winner, over {} decided games:", decided.len());
    let report = |label: &str, values: &mut Vec<i64>| {
        match median(values) {
            Some(mid) => println!(
                "  {label:<34} n={:<4} median {mid:>4} turns  (min {}, max {})",
                values.len(),
                values.first().copied().unwrap_or(0),
                values.last().copied().unwrap_or(0)
            ),
            None => println!("  {label:<34} n=0"),
        };
    };
    let mut lead_named_v = lead_named;
    let mut lead_seen_v = lead_seen;
    let mut lead_honest_v = lead_honest;
    let mut lead_urgent_v = lead_urgent;
    let mut meter_lag_v = meter_lag;
    report("warning: first denial → win", &mut lead_named_v);
    report("warning: AI meter ≥78 → win", &mut lead_seen_v);
    report("warning: honest meter ≥78 → win", &mut lead_honest_v);
    report("warning: urgent → win", &mut lead_urgent_v);
    report("meter lag (AI − honest)", &mut meter_lag_v);
    println!(
        "  {:<34} {blind} of {} wins ({:.0}%)",
        "nobody ever named the winner",
        decided.len(),
        100.0 * blind as f64 / decided.len().max(1) as f64
    );
    println!(
        "  {:<34} {never_seen} of {} wins ({:.0}%)",
        "AI meter never reached the bar",
        decided.len(),
        100.0 * never_seen as f64 / decided.len().max(1) as f64
    );
    println!(
        "  {:<34} {:.0}% before the alarm, {:.0}% after",
        "winner at war with a major",
        100.0 * war_before.0 as f64 / war_before.1.max(1) as f64,
        100.0 * war_after.0 as f64 / war_after.1.max(1) as f64
    );
    if !winning_lane.is_empty() {
        print!("  {:<34}", "lane the AI read on them");
        for (lane, count) in &winning_lane {
            print!(" {lane}={count}");
        }
        println!();
    }

    // Does being contested change anything? Read every major, not just the
    // winners: an alarm that works shows up as empires that got named and then
    // did *not* win. Compared against the base rate, one seat in `players`.
    let mut all = (0_usize, 0_usize);
    let mut named = (0_usize, 0_usize);
    let mut urgent = (0_usize, 0_usize);
    let mut contested = (0_usize, 0_usize);
    let mut gap_total = 0.0_f64;
    let mut gap_samples = 0_u32;
    for reading in &readings {
        for (pid, track) in &reading.tracks {
            let won = reading.winner == Some(*pid);
            all.0 += 1;
            all.1 += usize::from(won);
            if track.first_named.is_some() {
                named.0 += 1;
                named.1 += usize::from(won);
            }
            if track.first_urgent.is_some() {
                urgent.0 += 1;
                urgent.1 += usize::from(won);
            }
            // Named *and* somebody was actually at war with them afterwards:
            // the alarm plus a response, which is the thing being tested.
            if track.first_named.is_some() && track.war_turns_after > 0 {
                contested.0 += 1;
                contested.1 += usize::from(won);
            }
            gap_total += track.gap_total;
            gap_samples += track.gap_samples;
        }
    }
    let rate = |pair: (usize, usize)| {
        if pair.0 == 0 {
            "  n/a".to_string()
        } else {
            format!("{:5.1}%", 100.0 * pair.1 as f64 / pair.0 as f64)
        }
    };
    println!("\ndoes being contested stop them? (every major, every map)");
    println!("  {:<34} {} of {} → {}", "any major", all.1, all.0, rate(all));
    println!(
        "  {:<34} {} of {} → {}",
        "ever named by the denial layer",
        named.1,
        named.0,
        rate(named)
    );
    println!(
        "  {:<34} {} of {} → {}",
        "named and then fought",
        contested.1,
        contested.0,
        rate(contested)
    );
    println!(
        "  {:<34} {} of {} → {}",
        "clock read as urgent",
        urgent.1,
        urgent.0,
        rate(urgent)
    );
    println!(
        "\ninstrument gap (AI meter − victory screen), over {gap_samples} readings: {:+.1} points",
        gap_total / gap_samples.max(1) as f64
    );
}
