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
use civvis::rules::Yields;
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

/// Straight-line hex distance on the axial coordinates the engine uses. This
/// intentionally ignores world wrap: a settler journey that would be shorter
/// the other way round the globe reads as *longer* here, so the steps-to-
/// distance ratio this feeds is conservative about claiming a detour.
fn hex_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let (aq, ar) = (a.0 - (a.1 - (a.1 & 1)) / 2, a.1);
    let (bq, br) = (b.0 - (b.1 - (b.1 & 1)) / 2, b.1);
    let (dq, dr) = (aq - bq, ar - br);
    if (dq < 0) == (dr < 0) {
        (dq.abs() + dr.abs()) as u32
    } else {
        dq.abs().max(dr.abs()) as u32
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
    /// Turn the seat first held 2, 3, 4 … cities. `AdvancedAi` allows exactly
    /// one settler in existence empire-wide (`counts.settlers == 0` at
    /// `src/ai/advanced.rs:6700`), so if that clause is binding these arrive
    /// spaced by one build plus one walk rather than overlapping.
    founding_turns: Vec<u32>,
    /// Turns spent holding a settler while still short of the city target.
    settler_in_flight_turns: u32,
    /// Turns short of the target with a settler at the head of some city's
    /// queue — i.e. time spent *paying* for one rather than walking it.
    /// §12 left production as the remaining candidate for what gates the
    /// founding cadence; this is the column that decides it.
    settler_building_turns: u32,
    /// Of the turns a settler existed, how many it ended on the same tile it
    /// started. "Walking" is really "exists and has not founded" — a settler
    /// that is stationary is not travelling, it is waiting or dithering, and
    /// the two want completely different fixes.
    settler_idle_turns: u32,
    settler_moved_turns: u32,
    /// Last seen position of this seat's first settler, to see movement.
    last_settler_pos: Option<(i32, i32)>,
    /// Per-settler trace, keyed by unit id while the unit lives: spawn turn,
    /// spawn tile, last tile, tiles actually stepped. §13 showed transit is
    /// what gates the founding cadence but could not say whether the sites
    /// are far, the terrain slow, or the settler re-targeting; the ratio of
    /// steps to straight-line distance separates the third from the first two.
    live_settlers: BTreeMap<u32, (u32, (i32, i32), (i32, i32), u32)>,
    /// Target this settler was marching to when last seen, and how many times
    /// that target changed over its life. §14 left exactly one question: bad
    /// path, or changing destination?
    settler_aim: BTreeMap<u32, (i32, i32)>,
    aim_changes: u32,
    aim_samples: u32,
    aim_lost: u32,
    /// Why a settler that did not change tile did not change tile. §16 left
    /// this as the only unexplained fact in the transit story: in the control
    /// a settler stands still on 19% of its turns, and neither commitment nor
    /// re-targeting accounts for it.
    still_no_target: u32,
    still_spent: u32,
    still_crowded: u32,
    still_unexplained: u32,
    /// Finished journeys: (turns alive, steps taken, straight-line distance).
    settler_trips: Vec<(u32, u32, u32)>,
    /// Turns short of the city target with no settler anywhere.
    short_without_settler_turns: u32,
    /// Capital population, housing and food at fixed checkpoints. A settler
    /// costs a population and 80/110/140 production, so "why is expansion
    /// slow" reduces to "how fast does the capital grow" — and whether pop
    /// sits at the housing cap separates a housing constraint from a food one.
    checkpoints: Vec<(u32, i32, f64, f64, f64, f64, f64)>,
    cities_at_window: usize,
    /// Survival, tracked over the whole game rather than the window, because
    /// "recorded a short opening" is a proxy for early death and a proxy is
    /// not allowed to stand in for the thing once the thing is the headline.
    founded: bool,
    max_cities: usize,
    /// First turn the seat held no city after having held one.
    death_turn: Option<u32>,
    /// Ever lost the capital, whether or not the seat itself died.
    capital_lost: bool,
    had_capital: bool,
    cities_at_end: usize,
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
fn opening_of(
    mut game: Game,
    watch: usize,
    window: u32,
    depth: usize,
    civ_blind: bool,
) -> Vec<String> {
    let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
    for agent in fleet.iter_mut() {
        agent.civ_blind = civ_blind;
    }
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
    // Fires-check for the civilization-aware ablation: this probe is exactly
    // the measurement `civ_blind` is meant to collapse, so it is where the
    // flag has to prove it bites before any eval is spent on it.
    let civ_blind = args.iter().any(|arg| arg == "--civ-blind");
    let names = roster(number(args, "--roster", 21));

    println!(
        "opening_census --swap: seat {watch} replayed as each of {} civilizations on {maps} maps, \
         {players} players, {width}x{height}, window {window}, depth {depth}, seed {seed0}, \
         civ_blind {civ_blind}",
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
                openings.push((
                    civ.clone(),
                    sequence(&opening_of(game, watch, window, depth, civ_blind)),
                ));
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
    // Half the majors lose every city by turn ~29. Turning the barbarians off
    // is the one-flag experiment that says whether that is the barbarians or
    // the rivals, so it is a flag rather than a separate tool.
    let barbarians = number(&args, "--barbarians", 1) != 0;
    let parallel_settlers = args.iter().any(|arg| arg == "--parallel-settlers");
    let census_civ_blind = args.iter().any(|arg| arg == "--civ-blind");
    let settler_commit = args.iter().any(|arg| arg == "--settler-commit");
    let food_first = args
        .iter()
        .position(|arg| arg == "--food-bias")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    println!(
        "opening_census: {maps} maps x {players} players, {width}x{height}, {turns} turns, \
         window {window}, depth {depth}, seed {seed0}, barbarians {barbarians}, \
         parallel_settlers {parallel_settlers}"
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
            barbarians,
            ..GameOptions::new(players, width, height, seed, turns, 0)
        });
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        // Fires-check for the treatment: the cadence table above is what it
        // is meant to move, so it is the right place to prove it moves.
        for agent in fleet.iter_mut() {
            agent.parallel_settlers = parallel_settlers;
            agent.civ_blind = census_civ_blind;
            agent.food_first = food_first;
            agent.settler_commit = settler_commit;
        }
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
                // Movement must be read before EndTurn, which refreshes it.
                let settler_moves: Vec<(u32, f64, (i32, i32))> = game
                    .units
                    .values()
                    .filter(|u| u.owner == pid && u.kind == "settler")
                    .map(|u| (u.id, u.moves_left, (u.pos.0, u.pos.1)))
                    .collect();
                // A neighbouring tile holding any unit is a tile this settler
                // cannot step onto -- Civilization VI allows one unit per tile
                // per domain, so an empire's own escort blocks it as surely as
                // a rival does.
                let crowded: BTreeSet<u32> = settler_moves
                    .iter()
                    .filter(|(_, _, pos)| {
                        let here: civvis::Pos = (pos.0, pos.1);
                        game.nbrs(here)
                            .iter()
                            .any(|n| game.units.values().any(|u| u.pos == *n))
                    })
                    .map(|(id, _, _)| *id)
                    .collect();
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
                if !seats.contains_key(&pid) {
                    continue;
                }
                // Survival is tracked for the whole game, not the window.
                let live = game.cities.values().filter(|c| c.owner == pid).count();
                let holds_capital = game
                    .cities
                    .values()
                    .any(|city| city.owner == pid && city.is_capital);
                // `desired_cities` as `AdvancedAi::plan` computes it, so the
                // "short of target" columns below mean the same thing the
                // agent means by it.
                let land = game
                    .map
                    .tiles
                    .values()
                    .filter(|t| game.rules.is_passable(t) && !game.rules.is_water(t))
                    .count();
                let map_capacity = (2 + land / 55).clamp(3, 9);
                let cadence = game.standard_duration(90).max(1) as usize;
                let desired = (3 + game.turn as usize / cadence).min(map_capacity).min(6);
                let in_flight = game
                    .units
                    .values()
                    .filter(|u| u.owner == pid && u.kind == "settler")
                    .count();
                // Is any of this seat's cities currently paying for a settler?
                // Did the settler actually move this turn?
                let settler_pos = game
                    .units
                    .values()
                    .find(|u| u.owner == pid && u.kind == "settler")
                    .map(|u| (u.pos.0, u.pos.1));
                let settlers_now: Vec<(u32, (i32, i32))> = game
                    .units
                    .values()
                    .filter(|u| u.owner == pid && u.kind == "settler")
                    .map(|u| (u.id, (u.pos.0, u.pos.1)))
                    .collect();
                let aims_now: Vec<(u32, Option<(i32, i32)>)> = settlers_now
                    .iter()
                    .map(|(id, _)| {
                        (*id, fleet[pid].settler_target(*id).map(|p| (p.0, p.1)))
                    })
                    .collect();
                let fleet_aim_missing: BTreeSet<u32> = settlers_now
                    .iter()
                    .filter(|(id, _)| fleet[pid].settler_target(*id).is_none())
                    .map(|(id, _)| *id)
                    .collect();
                let building = game.cities.values().any(|c| {
                    c.owner == pid
                        && matches!(
                            c.queue.first(),
                            Some(Item::Unit { unit }) if unit == "settler"
                        )
                });
                {
                    let seat = seats.get_mut(&pid).expect("major seat recorded at setup");
                    if live > 0 {
                        seat.founded = true;
                    }
                    while seat.founding_turns.len() + 1 < live {
                        seat.founding_turns.push(turn + 1);
                    }
                    if live + in_flight < desired {
                        if in_flight > 0 {
                            seat.settler_in_flight_turns += 1;
                        } else if building {
                            seat.settler_building_turns += 1;
                        } else {
                            seat.short_without_settler_turns += 1;
                        }
                    } else if in_flight > 0 && live < desired {
                        seat.settler_in_flight_turns += 1;
                    }
                    if holds_capital {
                        seat.had_capital = true;
                    } else if seat.had_capital {
                        seat.capital_lost = true;
                    }
                    seat.max_cities = seat.max_cities.max(live);
                    if let Some(now) = settler_pos {
                        if seat.last_settler_pos == Some(now) {
                            seat.settler_idle_turns += 1;
                        } else if seat.last_settler_pos.is_some() {
                            seat.settler_moved_turns += 1;
                        }
                    }
                    seat.last_settler_pos = settler_pos;
                    for (id, moves, pos) in &settler_moves {
                        // Only classify a settler that did not change tile.
                        if seat.live_settlers.get(id).map(|e| e.2) != Some(*pos) {
                            continue;
                        }
                        if fleet_aim_missing.contains(id) {
                            seat.still_no_target += 1;
                        } else if *moves <= 0.0 {
                            seat.still_spent += 1;
                        } else if crowded.contains(id) {
                            seat.still_crowded += 1;
                        } else {
                            seat.still_unexplained += 1;
                        }
                    }
                    for (id, aim) in &aims_now {
                        seat.aim_samples += 1;
                        match (seat.settler_aim.get(id), aim) {
                            (Some(was), Some(now)) if was != now => {
                                seat.aim_changes += 1;
                                seat.settler_aim.insert(*id, *now);
                            }
                            (Some(_), None) => {
                                // The agent dropped this settler's target --
                                // one failed move does exactly that. Keep the
                                // old value rather than forgetting it: if the
                                // next target differs, that is a real
                                // re-targeting and must be counted as one.
                                seat.aim_lost += 1;
                            }
                            (None, Some(now)) => {
                                seat.settler_aim.insert(*id, *now);
                            }
                            _ => {}
                        }
                    }
                    for (id, pos) in &settlers_now {
                        let entry = seat
                            .live_settlers
                            .entry(*id)
                            .or_insert((turn + 1, *pos, *pos, 0));
                        if entry.2 != *pos {
                            entry.3 += 1;
                            entry.2 = *pos;
                        }
                    }
                    let gone: Vec<u32> = seat
                        .live_settlers
                        .keys()
                        .copied()
                        .filter(|id| !settlers_now.iter().any(|(live, _)| live == id))
                        .collect();
                    for id in gone {
                        if let Some((born, from, last, steps)) = seat.live_settlers.remove(&id) {
                            let straight = hex_distance(from, last);
                            seat.settler_trips.push((turn + 1 - born, steps, straight));
                        }
                    }
                    if matches!(turn + 1, 25 | 50 | 75 | 100) {
                        if let Some(cap) = game
                            .cities
                            .values()
                            .find(|c| c.owner == pid && c.is_capital)
                        {
                            let housing = game.city_housing(cap);
                            let food = game.city_yields(cap.id).food;
                            // The ceiling: what this same capital, on these
                            // same tiles, would work under appetites that
                            // want food. Nothing is adopted -- the engine
                            // still runs its own weights.
                            let now = game.city_yields(cap.id);
                            let greedy = game.city_yields_weighted(
                                cap.id,
                                Yields {
                                    food: 10.0,
                                    production: 1.0,
                                    gold: 0.1,
                                    science: 0.1,
                                    culture: 0.1,
                                    faith: 0.1,
                                },
                            );
                            seat.checkpoints.push((
                                turn + 1,
                                cap.pop,
                                housing,
                                food,
                                greedy.food,
                                now.production,
                                greedy.production,
                            ));
                        }
                    }
                    if live == 0 && seat.founded && seat.death_turn.is_none() {
                        seat.death_turn = Some(turn + 1);
                    }
                    seat.cities_at_end = live;
                }
                // Sample the capital right after this seat acted, which is
                // when the planner had its say. A city that completes two
                // items inside one turn shows only the second; in the opening
                // window nothing is that cheap.
                if turn >= window {
                    continue;
                }
                let head = game
                    .cities
                    .values()
                    .find(|city| city.owner == pid && city.is_capital)
                    .and_then(|city| city.queue.first())
                    .map(label);
                let cities = live;
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
    println!("\nper civilization, ranked by terminal score share");
    println!(
        "{:<14} {:>5} {:>8} {:>6} {:>17} {:>6} {:>7} {:>6}  {}",
        "civ", "seats", "distinct", "modal", "score share", "win%", "2ndcity", "cities", "opening"
    );
    // Collect first so the table can be ranked by outcome rather than by name.
    let mut civ_rows: Vec<(f64, String)> = Vec::new();
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
        let shares: Vec<f64> = rows.iter().map(|s| s.score_share).collect();
        let (share_mean, share_se) = mean_se(&shares);
        let wins = rows.iter().filter(|s| s.won).count();
        civ_rows.push((
            share_mean,
            format!(
                "{:<14} {:>5} {:>8} {:>5.0}% {:>9.4} +/- {:<5.4} {:>5.1}% {:>7} {:>6.2}  {}",
                civ,
                rows.len(),
                own.len(),
                100.0 * share,
                share_mean,
                share_se,
                100.0 * wins as f64 / rows.len().max(1) as f64,
                second_col,
                cities_mean,
                top
            ),
        ));
    }
    civ_rows.sort_by(|a, b| b.0.partial_cmp(&a.0).expect("finite score shares"));
    for (_, line) in &civ_rows {
        println!("{line}");
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

    // ---- 3a. what "short" actually means -------------------------------
    // The claim "half the seats die early" rested on the proxy above. A short
    // sequence could equally be a slow capital that lived to turn 500, so the
    // proxy is checked against measured survival rather than trusted.
    println!("\nwhat a short opening actually is (survival measured, not inferred)");
    println!(
        "{:<24} {:>6} {:>9} {:>10} {:>11} {:>12}",
        "group", "seats", "died", "cap lost", "alive@end", "score share"
    );
    for (name, group) in [
        (
            "full-depth opening",
            seats
                .iter()
                .filter(|s| s.builds.len() == depth)
                .collect::<Vec<_>>(),
        ),
        (
            "short opening",
            seats
                .iter()
                .filter(|s| s.builds.len() < depth)
                .collect::<Vec<_>>(),
        ),
    ] {
        let count = group.len().max(1);
        let died = group.iter().filter(|s| s.death_turn.is_some()).count();
        let cap = group.iter().filter(|s| s.capital_lost).count();
        let alive = group.iter().filter(|s| s.cities_at_end > 0).count();
        let shares: Vec<f64> = group.iter().map(|s| s.score_share).collect();
        let (share, _) = mean_se(&shares);
        println!(
            "{:<24} {:>6} {:>8.0}% {:>9.0}% {:>10.0}% {:>12.4}",
            name,
            group.len(),
            100.0 * died as f64 / count as f64,
            100.0 * cap as f64 / count as f64,
            100.0 * alive as f64 / count as f64,
            share
        );
    }
    let deaths: Vec<f64> = seats
        .iter()
        .filter_map(|s| s.death_turn.map(|t| t as f64))
        .collect();
    let (death_mean, death_se) = mean_se(&deaths);
    let never_founded = seats.iter().filter(|s| !s.founded).count();
    let early = deaths.iter().filter(|t| **t <= 100.0).count();
    println!(
        "\n{} of {n} seats lost every city; mean death turn {death_mean:.0} +/- {death_se:.0}, \
         {early} of them by turn 100. {never_founded} never founded at all.",
        deaths.len()
    );
    let peak: Vec<f64> = seats.iter().map(|s| s.max_cities as f64).collect();
    let (peak_mean, peak_se) = mean_se(&peak);
    let window_cities: Vec<f64> = seats.iter().map(|s| s.cities_at_window as f64).collect();
    let (window_mean, window_se) = mean_se(&window_cities);
    println!(
        "cities at turn {window}: {window_mean:.2} +/- {window_se:.2}   peak cities ever: \
         {peak_mean:.2} +/- {peak_se:.2}"
    );

    // ---- 3b. settlers in the opening -----------------------------------
    // `AdvancedAi` wants `(3 + turn/90).min(map_capacity).min(6)` cities, so
    // the opening's whole job is three cities by turn 90 — which takes two
    // settlers out of the capital. Count how many it actually queues.
    let settlers: Vec<f64> = seats
        .iter()
        .filter(|s| s.builds.len() == depth)
        .map(|s| s.builds.iter().filter(|b| *b == "settler").count() as f64)
        .collect();
    let (settler_mean, settler_se) = mean_se(&settlers);
    println!(
        "\nsettlers among the first {depth} capital builds: {settler_mean:.2} +/- {settler_se:.2} \
         over {} full-depth seats",
        settlers.len()
    );
    for wanted in 0..=2usize {
        let group: Vec<&Seat> = seats
            .iter()
            .filter(|s| s.builds.len() == depth)
            .filter(|s| s.builds.iter().filter(|b| *b == "settler").count() == wanted)
            .collect();
        if group.is_empty() {
            continue;
        }
        let shares: Vec<f64> = group.iter().map(|s| s.score_share).collect();
        let (share, se) = mean_se(&shares);
        let wins = group.iter().filter(|s| s.won).count();
        let peak: Vec<f64> = group.iter().map(|s| s.max_cities as f64).collect();
        let (peak_mean, _) = mean_se(&peak);
        println!(
            "  {wanted} settler(s): {:>4} seats, score {share:.4} +/- {se:.4}, {wins} wins \
             ({:.1}%), peak cities {peak_mean:.2}",
            group.len(),
            100.0 * wins as f64 / group.len() as f64
        );
    }

    // ---- 3c. is expansion serialized? ----------------------------------
    // `AdvancedAi` permits exactly one settler in existence empire-wide. If
    // that clause binds, cities arrive spaced by a build plus a walk however
    // many cities are already producing, and a seat spends most of its
    // shortfall watching a settler walk rather than building another.
    println!("\ncity founding cadence (turn the seat first held N cities)");
    println!("{:>3}  {:>7} {:>16}  {}", "N", "seats", "mean turn", "gap");
    let mut previous: Option<f64> = None;
    for step in 0..5usize {
        let turns_at: Vec<f64> = seats
            .iter()
            .filter_map(|s| s.founding_turns.get(step).map(|t| *t as f64))
            .collect();
        if turns_at.len() < 5 {
            break;
        }
        let (mean, se) = mean_se(&turns_at);
        let gap = previous.map(|p| mean - p);
        println!(
            "{:>3}  {:>7} {:>9.1} +/- {:<4.1}  {}",
            step + 2,
            turns_at.len(),
            mean,
            se,
            gap.map(|g| format!("+{g:.1}")).unwrap_or_default()
        );
        previous = Some(mean);
    }
    // Capital growth, which is what a settler is actually paid for.
    println!("\ncapital growth (a settler costs one population and 80/110/140 production)");
    println!(
        "{:>5} {:>7} {:>16} {:>16} {:>16} {:>16} {:>8} {:>7}",
        "turn", "seats", "population", "housing", "food yield", "food if greedy", "surplus", "prod"
    );
    for mark in [25u32, 50, 75, 100] {
        let rows: Vec<&(u32, i32, f64, f64, f64, f64, f64)> = seats
            .iter()
            .flat_map(|s| s.checkpoints.iter())
            .filter(|(turn, ..)| *turn == mark)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let pops: Vec<f64> = rows.iter().map(|(_, pop, ..)| *pop as f64).collect();
        let house: Vec<f64> = rows.iter().map(|(_, _, h, ..)| *h).collect();
        let food: Vec<f64> = rows.iter().map(|(_, _, _, f, ..)| *f).collect();
        let greedy: Vec<f64> = rows.iter().map(|(_, _, _, _, g, _, _)| *g).collect();
        let (greedy_mean, greedy_se) = mean_se(&greedy);
        let prod: Vec<f64> = rows.iter().map(|(_, _, _, _, _, p, _)| *p).collect();
        let gprod: Vec<f64> = rows.iter().map(|(.., gp)| *gp).collect();
        let (prod_mean, _) = mean_se(&prod);
        let (gprod_mean, _) = mean_se(&gprod);
        // Food consumption is two per population in Civilization VI, so the
        // surplus -- not the gross yield -- is what actually grows the city.
        let eaten = 2.0 * pops.iter().sum::<f64>() / pops.len().max(1) as f64;
        // "At cap" means the housing headroom is under one population, i.e.
        // growth is housing-bound rather than food-bound.
        let capped = rows
            .iter()
            .filter(|(_, pop, h, ..)| (*pop as f64) >= *h - 1.0)
            .count();
        let (pop_mean, pop_se) = mean_se(&pops);
        let (house_mean, house_se) = mean_se(&house);
        let (food_mean, food_se) = mean_se(&food);
        println!(
            "{:>5} {:>7} {:>9.2} +/- {:<4.2} {:>9.2} +/- {:<4.2} {:>9.2} +/- {:<4.2} \
             {:>9.2} +/- {:<4.2} {:>7} {:>13}",
            mark,
            rows.len(),
            pop_mean,
            pop_se,
            house_mean,
            house_se,
            food_mean,
            food_se,
            greedy_mean,
            greedy_se,
            format!(
                "{:.2}->{:.2}",
                food_mean - eaten,
                greedy_mean - eaten
            ),
            format!("{prod_mean:.2}->{gprod_mean:.2}")
        );
    }

    let flight: Vec<f64> = seats
        .iter()
        .map(|s| s.settler_in_flight_turns as f64)
        .collect();
    let idle: Vec<f64> = seats
        .iter()
        .map(|s| s.short_without_settler_turns as f64)
        .collect();
    let (flight_mean, flight_se) = mean_se(&flight);
    let (idle_mean, idle_se) = mean_se(&idle);
    let build: Vec<f64> = seats
        .iter()
        .map(|s| s.settler_building_turns as f64)
        .collect();
    let (build_mean, build_se) = mean_se(&build);
    let total = flight_mean + build_mean + idle_mean;
    println!(
        "\nwhere the time below the city target goes, per seat:\n  \
         {build_mean:6.1} +/- {build_se:<4.1} turns PAYING for a settler ({:.0}%)\n  \
         {flight_mean:6.1} +/- {flight_se:<4.1} turns WALKING one       ({:.0}%)\n  \
         {idle_mean:6.1} +/- {idle_se:<4.1} turns NEITHER            ({:.0}%)\n\
         If production gated the cadence the first row would dominate. It is the row that \
         decides whether the settler's 80/110/140 is the binding cost.",
        100.0 * build_mean / total.max(0.001),
        100.0 * flight_mean / total.max(0.001),
        100.0 * idle_mean / total.max(0.001)
    );
    let trips: Vec<(u32, u32, u32)> = seats
        .iter()
        .flat_map(|s| s.settler_trips.iter().copied())
        .collect();
    if !trips.is_empty() {
        let turns_v: Vec<f64> = trips.iter().map(|(t, ..)| *t as f64).collect();
        let steps_v: Vec<f64> = trips.iter().map(|(_, s, _)| *s as f64).collect();
        let dist_v: Vec<f64> = trips.iter().map(|(.., d)| *d as f64).collect();
        let (turns_m, turns_se) = mean_se(&turns_v);
        let (steps_m, steps_se) = mean_se(&steps_v);
        let (dist_m, dist_se) = mean_se(&dist_v);
        println!(
            "\nper settler journey ({} completed):\n  \
             {turns_m:.1} +/- {turns_se:.1} turns alive\n  \
             {steps_m:.1} +/- {steps_se:.1} tiles stepped\n  \
             {dist_m:.1} +/- {dist_se:.1} straight-line hexes from spawn to where it ended\n  \
             detour ratio {:.2} (steps / straight line)   pace {:.2} tiles per turn\n\
             A ratio near 1 means the settler goes where it was sent; well above 1 means it \
             re-targets en route. A pace near 1 with a settler's 2 movement means terrain, \
             not indecision.",
            trips.len(),
            steps_m / dist_m.max(0.01),
            steps_m / turns_m.max(0.01)
        );
    }
    let aim_changes: u32 = seats.iter().map(|s| s.aim_changes).sum();
    let aim_lost: u32 = seats.iter().map(|s| s.aim_lost).sum();
    let aim_samples: u32 = seats.iter().map(|s| s.aim_samples).sum();
    if aim_samples > 0 {
        println!(
            "\nsettler destination stability over {aim_samples} settler-turns:\n  \
             {aim_changes} turns ended aimed SOMEWHERE ELSE than the turn before ({:.1}%)\n  \
             {aim_lost} turns ended holding NO destination, having held one ({:.1}%)\n\
             A settler that chose a site and walked to it would show zero of both. \
             `AdvancedAi::settler_step` discards the target on any turn the unit fails to \
             move (src/ai/advanced.rs, `if !moved`), so the second row is a commitment \
             failure rather than a re-plan. Re-acquiring the same site costs only a search; \
             the first row is the one that costs distance.",
            100.0 * aim_changes as f64 / aim_samples as f64,
            100.0 * aim_lost as f64 / aim_samples as f64
        );
    }
    let no_t: u32 = seats.iter().map(|s| s.still_no_target).sum();
    let spent: u32 = seats.iter().map(|s| s.still_spent).sum();
    let crowd: u32 = seats.iter().map(|s| s.still_crowded).sum();
    let unex: u32 = seats.iter().map(|s| s.still_unexplained).sum();
    let still_total = (no_t + spent + crowd + unex).max(1);
    println!(
        "\nwhy a settler stood still ({still_total} such settler-turns):\n  \
         {no_t:6} held no destination at all               ({:.0}%)\n  \
         {spent:6} had spent its movement                   ({:.0}%)\n  \
         {crowd:6} every neighbouring tile was occupied     ({:.0}%)\n  \
         {unex:6} unexplained                               ({:.0}%)\n\
         Civilization VI allows one unit per tile per domain, so an empire's own escort \
         blocks a settler as surely as a rival does. 'Occupied' is a necessary condition \
         for congestion, not proof the settler wanted one of those tiles.",
        100.0 * no_t as f64 / still_total as f64,
        100.0 * spent as f64 / still_total as f64,
        100.0 * crowd as f64 / still_total as f64,
        100.0 * unex as f64 / still_total as f64
    );
    let moved: Vec<f64> = seats.iter().map(|s| s.settler_moved_turns as f64).collect();
    let stood: Vec<f64> = seats.iter().map(|s| s.settler_idle_turns as f64).collect();
    let (moved_mean, moved_se) = mean_se(&moved);
    let (stood_mean, stood_se) = mean_se(&stood);
    let settler_turns = moved_mean + stood_mean;
    println!(
        "\nof the turns a settler existed: {moved_mean:.1} +/- {moved_se:.1} MOVED ({:.0}%), \
         {stood_mean:.1} +/- {stood_se:.1} STOOD STILL ({:.0}%).\n\
         Standing still is not travel. If it dominates, the cadence is not paying for distance \
         — it is a settler that has nowhere it is willing to go, or is waiting for one.",
        100.0 * moved_mean / settler_turns.max(0.001),
        100.0 * stood_mean / settler_turns.max(0.001)
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
