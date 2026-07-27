//! What does each civilization actually open with, and does the civilization
//! change the answer?
//!
//! `docs/GENOME.md` has bounded the *scripted* opening twice: the four-slot
//! opening book sweeps to nothing (`opening_sweep`, holdout `-0.0019 +/-
//! 0.0148`) and deleting the book entirely costs `-0.003`. `order_ablate`
//! bounds technology and civic order below its 0.09 resolution. Every one of
//! those measurements pooled all civilizations together and asked whether a
//! *global* opening could be improved.
//!
//! Nobody has asked the prior question: **do different civilizations open
//! differently at all?** The planner is very nearly civilization-blind --
//! outside city-state types, a Greece culture bonus and a unique-unit
//! preference in `plan_production`, `g.players[pid].civ` does not reach the
//! decisions that make an opening. If every seat plays the same first six
//! builds regardless of who it is, then per-civilization opening play is not a
//! tuned-out lever, it is an **absent** one, and the bounded-to-zero results
//! above say nothing about it.
//!
//! This is the census that answers that. It plays whole games with the stock
//! fleet and records, per major seat:
//!
//! - the **build sequence** -- successive distinct items at the head of the
//!   capital's queue, inside an opening window, to a fixed depth;
//! - the **technology and civic sequence** -- the order they were actually
//!   acquired, same depth;
//! - **expansion tempo** -- the turn the second city appeared, and the city
//!   count at the end of the window;
//! - the **outcome** -- win, and terminal score share against the other
//!   majors.
//!
//! Three things come out, in the order they are worth reading:
//!
//! 1. **Divergence.** How many distinct openings exist, what share the single
//!    most common one takes, and how many of the civilizations name it as
//!    their own most common. A modal opening shared by every civilization is
//!    the civilization-blindness result, stated as a number.
//! 2. **Per-civilization table.** Each civilization's modal opening and how
//!    concentrated it is.
//! 3. **Opening against outcome.** Win rate and mean score share per opening,
//!    over the openings with enough seats to say anything.
//!
//! ```text
//! opening_census --players 6 --maps 24 --turns 500
//! ```
//!
//! ⚠ **Point 3 is correlational and confounded, by construction.** An agent
//! opens `settler -> warrior` *because* of the start it was given, so the
//! start quality is inside both the opening and the outcome. Read it as a
//! description of what the population looks like, never as "this opening wins"
//! -- the causal instrument is a paired `ai_eval` against a seat forced onto
//! the candidate opening, exactly as `opening_sweep`'s holdout is.
//!
//! Points 1 and 2 carry no such caveat: they are counts of what the agent did,
//! and an agent that plays one opening for twenty-one civilizations is a fact
//! about the agent, not about the maps.
//!
//! **Fires-check.** The tool prints how many seats recorded a full-depth
//! opening. `order_ablate` shipped once with an instrument that measured
//! nothing and read exactly like a settled subsystem; a census that observed
//! no builds would read exactly like perfect agreement. If `full-depth seats`
//! is not close to the seat count, the observation is broken, not the agent.
use std::collections::{BTreeMap, BTreeSet};

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions, Item};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// How an item reads in a sequence. Districts and wonders drop their position
/// so that two seats building a Campus in different places share an opening.
fn label(item: &Item) -> String {
    match item {
        Item::Formation { unit, formation } => format!("{unit}^{formation}"),
        Item::Unit { unit } => unit.clone(),
        Item::Building { building } => building.clone(),
        Item::District { district, .. } => format!("[{district}]"),
        Item::Wonder { wonder, .. } => format!("*{wonder}"),
        Item::Repair { repair, .. } => format!("repair:{repair}"),
        Item::Project { project } => format!("project:{project}"),
        Item::Product { product } => format!("product:{product}"),
    }
}

fn sequence(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(" > ")
    }
}

/// One major player's opening over one game.
#[derive(Clone, Default)]
struct Seat {
    civ: String,
    builds: Vec<String>,
    techs: Vec<String>,
    civics: Vec<String>,
    /// Head of the capital queue as last seen, so a change reads as a change.
    last_build: Option<String>,
    known_techs: BTreeSet<String>,
    known_civics: BTreeSet<String>,
    second_city_turn: Option<u32>,
    cities_at_window: usize,
    won: bool,
    score: i64,
    score_share: f64,
}

/// Mean and standard error of a sample, for any column that gets averaged.
fn mean_se(values: &[f64]) -> (f64, f64) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    (mean, (var / n as f64).sqrt())
}

/// The most common entry and its share, or `None` for an empty population.
fn modal(counts: &BTreeMap<String, usize>) -> Option<(String, usize, f64)> {
    let total: usize = counts.values().sum();
    counts
        .iter()
        .max_by_key(|(key, count)| (**count, std::cmp::Reverse(key.as_str().len())))
        .map(|(key, count)| (key.clone(), *count, *count as f64 / total.max(1) as f64))
}

fn tally<'a>(items: impl Iterator<Item = &'a String>) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        *counts.entry(item.clone()).or_default() += 1;
    }
    counts
}

/// The stock major roster, in the order `seat_civs` hands it out.
fn roster(size: usize) -> Vec<String> {
    let probe = Game::new(size, 24, 16, 1, 2, 0);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut names = Vec::new();
    for pid in 0..probe.players.len() {
        if probe.players[pid].is_minor {
            continue;
        }
        let civ = probe.players[pid].civ.clone();
        if seen.insert(civ.clone()) {
            names.push(civ);
        }
    }
    names
}

/// Play `window` turns and return seat `watch`'s capital build sequence.
fn opening_of(mut game: Game, watch: usize, window: u32, depth: usize) -> Vec<String> {
    let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
    let mut builds: Vec<String> = Vec::new();
    let mut last: Option<String> = None;
    for _ in 0..window {
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
            if pid != watch || builds.len() >= depth {
                continue;
            }
            let head = game
                .cities
                .values()
                .find(|city| city.owner == watch && city.is_capital)
                .and_then(|city| city.queue.first())
                .map(label);
            if let Some(name) = head {
                if last.as_deref() != Some(name.as_str()) {
                    builds.push(name.clone());
                    last = Some(name);
                }
            }
        }
    }
    builds
}

/// Does the civilization change the opening *at all*?
///
/// Hold the map, the seed, the rivals and the start fixed; overwrite one
/// seat's civilization name and replay the opening window. This is the
/// `search_probe` idiom -- one flag, one agent, one position, so it is paired
/// by construction and confounds nothing.
///
/// The overwrite happens **after** construction, so the seat keeps the start
/// tile it was assigned. That deliberately gives up Civilization VI's start
/// bias in exchange for isolating the decision layer: any divergence here is
/// the planner reading the civilization, and nothing else.
fn swap_probe(args: &[String]) {
    let players = number(args, "--players", 6);
    let width = number(args, "--width", 24) as i32;
    let height = number(args, "--height", 16) as i32;
    let maps = number(args, "--maps", 6);
    let seed0 = number(args, "--seed", 300_000) as u64;
    let jobs = number(args, "--jobs", parallel::default_jobs());
    let depth = number(args, "--depth", 6);
    let window = number(args, "--window", 40) as u32;
    let watch = number(args, "--watch", 0);
    let names = roster(number(args, "--roster", 21));

    println!(
        "opening_census --swap: seat {watch} replayed as each of {} civilizations on {maps} maps, \
         {players} players, {width}x{height}, window {window}, depth {depth}, seed {seed0}",
        names.len()
    );

    let rows = parallel::map(maps, jobs, {
        let names = names.clone();
        move |index| {
            let seed = seed0 + index as u64;
            let base = Game::new(players, width, height, seed, window + 1, 0);
            let mut openings: Vec<(String, String)> = Vec::new();
            for civ in &names {
                let mut game = base.clone();
                if let Some(seat) = game.players.get_mut(watch) {
                    seat.civ = civ.clone();
                }
                openings.push((civ.clone(), sequence(&opening_of(game, watch, window, depth))));
            }
            openings
        }
    });

    let mut total_distinct = 0usize;
    let mut identical_maps = 0usize;
    println!("\n{:<6} {:>8}  {}", "map", "distinct", "openings seen");
    for (index, openings) in rows.iter().enumerate() {
        let distinct: BTreeSet<&str> = openings.iter().map(|(_, seq)| seq.as_str()).collect();
        total_distinct += distinct.len();
        if distinct.len() == 1 {
            identical_maps += 1;
        }
        let shown: Vec<&str> = distinct.iter().copied().take(3).collect();
        println!(
            "{:<6} {:>8}  {}{}",
            seed0 + index as u64,
            distinct.len(),
            shown.join("  |  "),
            if distinct.len() > 3 { "  | …" } else { "" }
        );
    }
    let maps_seen = rows.len().max(1);
    println!(
        "\n{identical_maps} of {maps_seen} maps: every civilization opened identically.\n\
         mean distinct openings per map: {:.2} out of {} civilizations.",
        total_distinct as f64 / maps_seen as f64,
        names.len()
    );
    println!(
        "\nA mean of 1.00 is civilization-blindness, proven directly: no decision in the opening \
         reads who the seat is.\nAnything above 1.00 is the planner's existing civilization-aware \
         code firing, and the rows above name which civilizations it separates."
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--swap") {
        swap_probe(&args);
        return;
    }
    let players = number(&args, "--players", 6);
    let maps = number(&args, "--maps", 24);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 300_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let depth = number(&args, "--depth", 6);
    let window = number(&args, "--window", 60) as u32;

    println!(
        "opening_census: {maps} maps x {players} players, {width}x{height}, {turns} turns, \
         window {window}, depth {depth}, seed {seed0}"
    );

    let games = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        // `seat_civs` hands the stock roster out in seat order, so without
        // this every map seats Rome at 0 and Egypt at 1 and a per-civilization
        // table would really be a per-seat table. Shuffling breaks that
        // confound; start bias still applies, so the civilization keeps
        // whatever terrain preference Civilization VI gives it.
        let mut game = Game::new_with(GameOptions {
            randomize_civs: true,
            ..GameOptions::new(players, width, height, seed, turns, 0)
        });
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor)
            .collect();
        let mut seats: BTreeMap<usize, Seat> = majors
            .iter()
            .map(|pid| {
                let mut seat = Seat {
                    civ: game.players[*pid].civ.clone(),
                    ..Seat::default()
                };
                seat.known_techs = game.players[*pid].techs.clone();
                seat.known_civics = game.players[*pid].civics.clone();
                (*pid, seat)
            })
            .collect();

        for turn in 0..turns {
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
                // Sample the capital right after this seat acted, which is
                // when the planner had its say. A city that completes two
                // items inside one turn shows only the second; in the opening
                // window nothing is that cheap.
                if turn >= window || !seats.contains_key(&pid) {
                    continue;
                }
                let head = game
                    .cities
                    .values()
                    .find(|city| city.owner == pid && city.is_capital)
                    .and_then(|city| city.queue.first())
                    .map(label);
                let cities = game.cities.values().filter(|c| c.owner == pid).count();
                let techs = game.players[pid].techs.clone();
                let civics = game.players[pid].civics.clone();
                let seat = seats.get_mut(&pid).expect("major seat recorded at setup");
                if let Some(name) = head {
                    if seat.last_build.as_deref() != Some(name.as_str()) {
                        if seat.builds.len() < depth {
                            seat.builds.push(name.clone());
                        }
                        seat.last_build = Some(name);
                    }
                }
                for tech in techs.difference(&seat.known_techs.clone()) {
                    if seat.techs.len() < depth {
                        seat.techs.push(tech.clone());
                    }
                    seat.known_techs.insert(tech.clone());
                }
                for civic in civics.difference(&seat.known_civics.clone()) {
                    if seat.civics.len() < depth {
                        seat.civics.push(civic.clone());
                    }
                    seat.known_civics.insert(civic.clone());
                }
                if cities >= 2 && seat.second_city_turn.is_none() {
                    seat.second_city_turn = Some(turn + 1);
                }
                seat.cities_at_window = cities;
            }
        }

        let total: i64 = majors.iter().map(|pid| game.score(*pid).max(0)).sum();
        for (pid, seat) in seats.iter_mut() {
            seat.score = game.score(*pid);
            seat.score_share = seat.score.max(0) as f64 / total.max(1) as f64;
            seat.won = game.winner == Some(*pid);
        }
        seats.into_values().collect::<Vec<Seat>>()
    });

    let all: Vec<Seat> = games.into_iter().flatten().collect();
    // A seat that never held a capital inside the window recorded nothing.
    // Pooling those with real openings makes a one-item sequence the modal
    // opening and reads as agreement; they are absence of data instead.
    let dead = all.iter().filter(|s| s.builds.is_empty()).count();
    let seats: Vec<Seat> = all.iter().filter(|s| !s.builds.is_empty()).cloned().collect();
    if seats.is_empty() {
        println!("no major seat held a capital inside the window");
        return;
    }
    println!(
        "\n{} of {} seats never held a capital inside the window and are excluded",
        dead,
        all.len()
    );
    let n = seats.len();
    let full_depth = seats.iter().filter(|s| s.builds.len() == depth).count();
    println!(
        "\nseats {n}   full-depth openings {full_depth} ({:.1}%)   \
         fires-check: a low share means the observation is broken, not the agent",
        100.0 * full_depth as f64 / n as f64
    );

    // ---- 1. divergence -------------------------------------------------
    let build_key: Vec<String> = seats.iter().map(|s| sequence(&s.builds)).collect();
    let tech_key: Vec<String> = seats.iter().map(|s| sequence(&s.techs)).collect();
    let civic_key: Vec<String> = seats.iter().map(|s| sequence(&s.civics)).collect();

    let mut civs: BTreeSet<&str> = BTreeSet::new();
    for seat in &seats {
        civs.insert(seat.civ.as_str());
    }

    for (name, keys) in [
        ("build", &build_key),
        ("tech", &tech_key),
        ("civic", &civic_key),
    ] {
        let counts = tally(keys.iter());
        let (top, count, share) = modal(&counts).expect("at least one seat");
        // How many civilizations name the pooled modal sequence as their own
        // most common one. This is the civilization-blindness number.
        let mut agreeing = 0usize;
        for civ in &civs {
            let own = tally(
                seats
                    .iter()
                    .zip(keys.iter())
                    .filter(|(seat, _)| seat.civ == *civ)
                    .map(|(_, key)| key),
            );
            if let Some((civ_top, _, _)) = modal(&own) {
                if civ_top == top {
                    agreeing += 1;
                }
            }
        }
        println!(
            "\n{name}: {} distinct sequences over {n} seats; modal takes {count} ({:.1}%)\n  \
             modal = {top}\n  {agreeing} of {} civilizations name it as their own modal sequence",
            counts.len(),
            100.0 * share,
            civs.len()
        );
    }

    // ---- 2. per-civilization -------------------------------------------
    println!("\nper civilization (modal capital opening)");
    println!(
        "{:<14} {:>5} {:>7} {:>6}  {:>7} {:>6}  {}",
        "civ", "seats", "distinct", "modal", "2ndcity", "cities", "opening"
    );
    for civ in &civs {
        let rows: Vec<&Seat> = seats.iter().filter(|s| s.civ == *civ).collect();
        let own = tally(
            seats
                .iter()
                .zip(build_key.iter())
                .filter(|(seat, _)| seat.civ == *civ)
                .map(|(_, key)| key),
        );
        let (top, _, share) = match modal(&own) {
            Some(value) => value,
            None => continue,
        };
        let second: Vec<f64> = rows
            .iter()
            .filter_map(|s| s.second_city_turn.map(|t| t as f64))
            .collect();
        let (second_mean, _) = mean_se(&second);
        // No seat founded a second city inside the window. Printing the mean
        // of nothing as 0.0 reads as "second city on turn zero", which is the
        // opposite of what happened.
        let second_col = if second.is_empty() {
            "never".to_string()
        } else {
            format!("{second_mean:.1}")
        };
        let cities: Vec<f64> = rows.iter().map(|s| s.cities_at_window as f64).collect();
        let (cities_mean, _) = mean_se(&cities);
        println!(
            "{:<14} {:>5} {:>8} {:>5.0}%  {:>7} {:>6.2}  {}",
            civ,
            rows.len(),
            own.len(),
            100.0 * share,
            second_col,
            cities_mean,
            top
        );
    }

    // ---- 3. opening against outcome ------------------------------------
    // Name the confound with a number before using the table it distorts.
    let by_length: Vec<f64> = seats
        .iter()
        .filter(|s| s.builds.len() == depth)
        .map(|s| s.score_share)
        .collect();
    let short: Vec<f64> = seats
        .iter()
        .filter(|s| s.builds.len() < depth)
        .map(|s| s.score_share)
        .collect();
    let (long_mean, long_se) = mean_se(&by_length);
    let (short_mean, short_se) = mean_se(&short);
    println!(
        "\nsequence length is early survival: seats recording all {depth} builds score \
         {long_mean:.4} +/- {long_se:.4} ({} seats); seats recording fewer score \
         {short_mean:.4} +/- {short_se:.4} ({} seats)",
        by_length.len(),
        short.len()
    );

    let min_seats = number(&args, "--min-seats", 8);
    println!(
        "\nopening against outcome (>= {min_seats} seats) -- CORRELATIONAL, the start is inside \
         both columns"
    );
    // Full-depth sequences only. A seat whose capital fell on turn 12 records
    // two builds and then scores nothing, so *sequence length is a proxy for
    // early survival* -- pool the short ones in and the table ranks openings
    // by how long their owner lived, which is exactly backwards.
    println!(
        "restricted to the {full_depth} seats that recorded all {depth} builds, because a short \
         sequence means an early death and would rank survival instead"
    );
    println!(
        "{:<6} {:>6} {:>16} {:>7}  {}",
        "seats", "wins", "score share", "win%", "opening"
    );
    let counts = tally(
        seats
            .iter()
            .zip(build_key.iter())
            .filter(|(seat, _)| seat.builds.len() == depth)
            .map(|(_, key)| key),
    );
    let mut rows: Vec<(usize, usize, f64, f64, String)> = Vec::new();
    for (key, count) in &counts {
        if *count < min_seats {
            continue;
        }
        let group: Vec<&Seat> = seats
            .iter()
            .zip(build_key.iter())
            .filter(|(seat, k)| *k == key && seat.builds.len() == depth)
            .map(|(seat, _)| seat)
            .collect();
        let shares: Vec<f64> = group.iter().map(|s| s.score_share).collect();
        let (share, se) = mean_se(&shares);
        let wins = group.iter().filter(|s| s.won).count();
        rows.push((*count, wins, share, se, key.clone()));
    }
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).expect("finite score shares"));
    for (count, wins, share, se, key) in &rows {
        println!(
            "{:<6} {:>6} {:>8.4} +/- {:<5.4} {:>6.1}%  {}",
            count,
            wins,
            share,
            se,
            100.0 * *wins as f64 / *count as f64,
            key
        );
    }
    if rows.is_empty() {
        println!("(no opening reached {min_seats} seats -- the population is that dispersed)");
    }
}
