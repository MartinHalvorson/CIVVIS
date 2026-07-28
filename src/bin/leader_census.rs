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
//! `docs/COUNTERING_LEADERS.md` carries what it has measured, including which
//! of these readings reversed once the census was run at the map size the
//! exhibition actually deploys.
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, GrandStrategy, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use std::collections::{BTreeMap, BTreeSet};

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
    /// The most majors ever at war with it at once, and the most after the
    /// first alarm. A denial layer that works looks like a dogpile; one that
    /// does not looks like a series of duels.
    peak_belligerents: usize,
    peak_belligerents_after: usize,
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

/// Candidate early-warning instruments, read on every major every turn. The
/// question they answer together: at turn `end - K`, does any of them already
/// put the eventual winner on top? An alarm can only ever be as early as the
/// earliest signal that ranks correctly.
const SIGNALS: [&str; 9] = [
    "victory_threat",
    "AI meter",
    "score",
    "cities",
    "techs",
    "military",
    "faith",
    "tourists",
    "religion race",
];

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
    /// Mean gold held per living-major-turn, and the share of those turns
    /// spent under each grand strategy. `advanced_gold_spending` keys its
    /// treasury reserve off the plan -- 75+25/city under Conquest or Recovery
    /// against 250-300+50-75/city under the builder strategies -- so a
    /// treatment that changes the plan mix moves the gold held without
    /// anything being saved or wasted.
    gold_total: f64,
    gold_samples: u64,
    strategy_turns: BTreeMap<&'static str, u64>,
    /// Distinct (observer, target) pairs the denial layer ever named, and how
    /// many of those observers ever actually went to war with the empire they
    /// named.
    named_pairs: usize,
    followed_pairs: usize,
    /// `signals[turn][major_index][signal]`, with `majors` naming the seats.
    signals: Vec<Vec<[f64; SIGNALS.len()]>>,
    majors: Vec<usize>,
}

/// Which seat leads on `signal` at `turn`, or `None` when the table is empty
/// or every reading is identical (a tie carries no information).
fn leader_on(reading: &MapReading, turn: usize, signal: usize) -> Option<usize> {
    let row = reading.signals.get(turn)?;
    let mut best = f64::NEG_INFINITY;
    let mut who = None;
    let mut ties = 0;
    for (index, values) in row.iter().enumerate() {
        let value = values[signal];
        if value > best {
            best = value;
            who = Some(reading.majors[index]);
            ties = 1;
        } else if value == best {
            ties += 1;
        }
    }
    (ties == 1).then_some(who).flatten()
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
    // The deployed exhibition seats 9 city-states at 6 players (74x46). A
    // census run without them is measuring a different game -- city-states
    // carry envoys, suzerainty and a large share of the religious map.
    let city_states = number(&args, "--city-states", 0);
    // Which response shape the whole table plays. The census is otherwise
    // identical, so `--arm` reads what a treatment does to behaviour before
    // any question of what it does to strength.
    let arm = args
        .iter()
        .position(|a| a == "--arm")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or("ship")
        .to_string();
    if !matches!(
        arm.as_str(),
        "ship" | "in_lane" | "stand_down" | "early" | "early_build"
    ) {
        eprintln!("--arm must be ship, in_lane, stand_down, early or early_build");
        std::process::exit(2);
    }
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "leader_census: {maps} maps, {players}p {width}x{height}, {city_states} city-states, \
         {turns} turns, seed {seed0}, arm {arm}, denial bar {DENIAL_BAR}"
    );

    let arm_label = arm.clone();
    let readings = parallel::map(maps, jobs, move |index| {
        let arm = arm_label.as_str();
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, city_states);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        for planner in fleet.iter_mut() {
            planner.counter_in_lane = arm == "in_lane" || arm == "early_build";
            planner.counter_stand_down = arm == "stand_down";
            planner.early_score_alarm = arm == "early" || arm == "early_build";
        }
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
        let mut signals: Vec<Vec<[f64; SIGNALS.len()]>> = Vec::with_capacity(turns as usize);
        let mut named_by_turn: Vec<(u32, usize)> = Vec::new();
        let mut named_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut gold_total = 0.0_f64;
        let mut gold_samples = 0_u64;
        let mut strategy_turns: BTreeMap<&'static str, u64> = BTreeMap::new();
        let mut followed_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
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

            let leading_score = majors
                .iter()
                .map(|pid| game.score(*pid))
                .max()
                .unwrap_or(0);
            let mut row = vec![[0.0_f64; SIGNALS.len()]; majors.len()];

            // Who is close, on each instrument.
            for (slot, target) in majors.iter().copied().enumerate() {
                if !game.players[target].alive {
                    continue;
                }
                let (lane, seen) = probe.rival_pressure(&game, target);
                let honest = game.victory_threat(target);
                let player = &game.players[target];
                row[slot] = [
                    honest,
                    seen as f64,
                    game.score(target) as f64,
                    game.player_city_ids(target).len() as f64,
                    player.techs.len() as f64,
                    game.military_power(target),
                    player.faith,
                    game.domestic_tourists(target) as f64,
                    game.victory_races(target, leading_score).religious,
                ];
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

            // What the treasury is doing, and under which plan.
            for observer in majors.iter().copied() {
                if !game.players[observer].alive {
                    continue;
                }
                gold_total += game.players[observer].gold;
                gold_samples += 1;
                if let Some(plan) = fleet[observer].current_plan() {
                    *strategy_turns.entry(plan.strategy.as_str()).or_default() += 1;
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
                    named_pairs.insert((observer, rival));
                    let track = tracks.get_mut(&rival).expect("major tracked");
                    note(&mut track.first_named, turn);
                }
            }

            // Naming a rival is a decision; declaring on them is the act. The
            // gap between the two is what a response layer is actually worth
            // before any question of whether the war converts.
            for (observer, rival) in named_pairs.iter().copied() {
                if game.is_at_war(observer, rival) {
                    followed_pairs.insert((observer, rival));
                }
            }

            // Did anybody actually go to war with them, and when relative to
            // the first alarm.
            for target in majors.iter().copied() {
                if !game.players[target].alive {
                    continue;
                }
                let belligerents = majors
                    .iter()
                    .filter(|other| **other != target && game.is_at_war(target, **other))
                    .count();
                let at_war = belligerents > 0;
                let track = tracks.get_mut(&target).expect("major tracked");
                track.peak_belligerents = track.peak_belligerents.max(belligerents);
                let after = track.first_named.is_some_and(|first| turn >= first);
                if after {
                    track.peak_belligerents_after =
                        track.peak_belligerents_after.max(belligerents);
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
            signals.push(row);
        }

        for target in majors.iter().copied() {
            let namers = named_pairs
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
            signals,
            majors,
            named_pairs: named_pairs.len(),
            followed_pairs: followed_pairs.len(),
            gold_total,
            gold_samples,
            strategy_turns,
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

    // Naming a rival is a decision; declaring on them is the act; several
    // empires declaring at once is the only thing that looks like a coalition.
    // Each step can lose the whole layer, so measure all three.
    let named_pairs: usize = readings.iter().map(|r| r.named_pairs).sum();
    let followed_pairs: usize = readings.iter().map(|r| r.followed_pairs).sum();
    println!(
        "\nfollow-through: {followed_pairs} of {named_pairs} (observer, target) pairs \
         went to war with the empire they named ({:.0}%)",
        100.0 * followed_pairs as f64 / named_pairs.max(1) as f64
    );

    let gold_total: f64 = readings.iter().map(|r| r.gold_total).sum();
    let gold_samples: u64 = readings.iter().map(|r| r.gold_samples).sum();
    let mut strategy_turns: BTreeMap<&str, u64> = BTreeMap::new();
    for reading in &readings {
        for (name, count) in &reading.strategy_turns {
            *strategy_turns.entry(name).or_default() += count;
        }
    }
    let strategy_total: u64 = strategy_turns.values().sum();
    println!(
        "\ntreasury: mean {:.0} gold per living-major-turn over {gold_samples} samples",
        gold_total / gold_samples.max(1) as f64
    );
    print!("plan mix:");
    for (name, count) in &strategy_turns {
        print!(
            " {name}={:.0}%",
            100.0 * *count as f64 / strategy_total.max(1) as f64
        );
    }
    println!();

    let mut dogpile: BTreeMap<usize, usize> = BTreeMap::new();
    let mut namers: BTreeMap<usize, usize> = BTreeMap::new();
    let mut lone = 0_usize;
    for reading in &decided {
        let winner = reading.winner.expect("decided");
        let Some(track) = reading.tracks.get(&winner) else {
            continue;
        };
        *dogpile.entry(track.peak_belligerents_after).or_default() += 1;
        *namers.entry(track.namers).or_default() += 1;
        if track.peak_belligerents_after <= 1 {
            lone += 1;
        }
    }
    print!("\nmajors at war with the winner at once, after the alarm:");
    for (count, games) in &dogpile {
        print!(" {count}→{games}");
    }
    println!(
        "\n  {lone} of {} wins ({:.0}%) never faced more than one at a time",
        decided.len(),
        100.0 * lone as f64 / decided.len().max(1) as f64
    );
    print!("distinct rivals that ever named the winner:");
    for (count, games) in &namers {
        print!(" {count}→{games}");
    }
    println!();

    // Before anybody builds machinery to organise a dogpile, ask whether a
    // dogpile does anything. Every major, bucketed by the most rivals that
    // were ever at war with it at once, against the base rate.
    let mut by_pile: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for reading in &readings {
        for (pid, track) in &reading.tracks {
            let bucket = track.peak_belligerents.min(3);
            let slot = by_pile.entry(bucket).or_default();
            slot.0 += 1;
            slot.1 += usize::from(reading.winner == Some(*pid));
        }
    }
    println!("\nwin rate by the most rivals ever at war with them at once:");
    for (bucket, (seats, wins)) in &by_pile {
        let label = if *bucket == 3 {
            "3+".to_string()
        } else {
            bucket.to_string()
        };
        println!(
            "  {label:<3} {wins:>4} of {seats:<4} → {:5.1}%",
            100.0 * *wins as f64 / (*seats).max(1) as f64
        );
    }

    // Could anybody have known earlier? At `end - K`, does any instrument
    // already put the eventual winner on top? An alarm can be no earlier than
    // the earliest signal that ranks correctly, so this bounds what any
    // response mechanism could possibly be given to work with.
    let leads = [25_usize, 50, 100, 150, 200];
    println!("\nwho leads at end − K, and is it the eventual winner?");
    print!("  {:<16}", "signal");
    for lead in leads {
        print!(" K={lead:<7}");
    }
    println!("   settles at");
    for (signal, name) in SIGNALS.iter().enumerate() {
        print!("  {name:<16}");
        for lead in leads {
            let mut hits = 0_usize;
            let mut total = 0_usize;
            for reading in &decided {
                let winner = reading.winner.expect("decided");
                let Some(turn) = (reading.end_turn as usize).checked_sub(lead) else {
                    continue;
                };
                total += 1;
                if leader_on(reading, turn, signal) == Some(winner) {
                    hits += 1;
                }
            }
            if total == 0 {
                print!(" {:<9}", "  n/a");
            } else {
                print!(" {:>4.0}% n={total:<3}", 100.0 * hits as f64 / total as f64);
            }
        }
        // The turn from which the winner leads on this signal and never gives
        // the lead back: the point the game was decided, if anybody was
        // reading this instrument.
        let mut settles: Vec<i64> = Vec::new();
        for reading in &decided {
            let winner = reading.winner.expect("decided");
            let end = reading.signals.len();
            let mut settled = end;
            for turn in (0..end).rev() {
                match leader_on(reading, turn, signal) {
                    Some(leader) if leader == winner => settled = turn,
                    Some(_) => break,
                    None => break,
                }
            }
            if settled < end {
                settles.push((end - settled) as i64);
            }
        }
        match median(&mut settles) {
            Some(mid) => println!("   {mid:>4} before the end (n={})", settles.len()),
            None => println!("   never"),
        }
    }
    println!(
        "  base rate for a {players}-seat table is {:.0}%; the denial layer's own \
         alarm arrives with a median 16-turn lead.",
        100.0 / players.max(1) as f64
    );
}
