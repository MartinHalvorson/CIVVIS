//! What do Ages actually do in a CIVVIS game, and is a Dark Age ever worth
//! having?
//!
//! Nothing in the repository has ever looked. The engine has carried Era Score,
//! four ages, twelve Dedications and seven Dark Age cards since 2026-07-23, and
//! no measurement says how often each age occurs, how close the misses are, or
//! whether a civilization that spends an era in the dark ends the game ahead or
//! behind. Every claim about age strategy is therefore a guess, including the
//! folk one this exists to test: that a Dark Age is a *setup*, taken on purpose,
//! because escaping one straight to the Golden threshold is a Heroic Age with
//! three Dedications instead of one.
//!
//! Per major seat, per age transition, this records the age entered, the Era
//! Score held against the two thresholds it was judged by, and the Dedication
//! chosen. It then grades that choice: `projected_dedication_score` says what
//! every *offered* Dedication would have paid over the era just ended, so the
//! chosen one has a rank and a regret against the best available.
//!
//! Reported:
//!
//! - the age distribution, per era and overall;
//! - **near misses** — how much Era Score a Dark Age was short by, and how
//!   much a Normal Age fell short of Golden. A distribution bunched just under
//!   a threshold means the age is decided by a handful of moments and is worth
//!   playing for; one spread wide means it is decided by the whole era;
//! - **Heroic** frequency, and how many of them came from a Dark Age that
//!   could instead have reached Normal — the intentional-Dark-Age question in
//!   its only falsifiable form;
//! - **outcome by age history**: win rate and mean final rank against the
//!   number of dark and golden ages a seat held, and against the age it held
//!   in each era;
//! - **dedication regret**: the share of choices that took the best available
//!   Dedication, and the Era Score left on the table.
//!
//! ```text
//! age_census --players 4 --maps 24 --turns 500
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, Weights};
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

/// One civilization's arrival in one age.
#[derive(Clone)]
struct Entry {
    era: usize,
    turn: u32,
    age: String,
    /// Era Score held when the age was judged, and the two thresholds it was
    /// judged against.
    score: i64,
    normal: i64,
    golden: i64,
    /// The Dedication taken, its rank among those offered by projected score,
    /// how many were offered, and the score of the best one.
    chosen: Vec<String>,
    rank: usize,
    offered: usize,
    best_projection: i64,
    chosen_projection: i64,
}

/// A seat's whole game.
#[derive(Clone)]
struct Seat {
    entries: Vec<Entry>,
    won: bool,
    rank: usize,
    seats: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 900_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "age_census: {maps} maps, {players}p {width}x{height}, {turns} turns, seed {seed0}"
    );

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();
        let mut seats: BTreeMap<usize, Vec<Entry>> = BTreeMap::new();
        let mut last_age: BTreeMap<usize, String> = majors
            .iter()
            .map(|pid| (*pid, game.players[*pid].age.clone()))
            .collect();
        let mut last_era = game.world_era;
        let mut pending: Vec<(usize, Entry, Vec<(String, i64)>)> = Vec::new();

        for turn in 0..turns {
            if game.winner.is_some() {
                break;
            }
            // The state each seat is judged on is the state it holds the turn
            // before the transition, so it has to be read before the turn runs.
            let before: BTreeMap<usize, (i64, i64, i64)> = majors
                .iter()
                .map(|pid| {
                    let p = &game.players[*pid];
                    (
                        *pid,
                        (p.era_score, p.normal_age_threshold, p.golden_age_threshold),
                    )
                })
                .collect();

            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }

            // A seat dedicates on its own turn, which for most of the table is
            // *after* the transition lands, so a pending record is settled one
            // full turn later. The projection catalogue is captured at the
            // transition, when `last_era_triggers` still names the era that
            // paid for it.
            for (pid, mut entry, catalogue) in std::mem::take(&mut pending) {
                let chosen: Vec<String> = game.players[pid].dedications.iter().cloned().collect();
                let taken = chosen.first().cloned().unwrap_or_default();
                entry.rank = catalogue
                    .iter()
                    .position(|(name, _): &(String, i64)| *name == taken)
                    .unwrap_or(catalogue.len());
                entry.chosen_projection = catalogue
                    .iter()
                    .find(|(name, _)| *name == taken)
                    .map_or(0, |entry| entry.1);
                entry.chosen = chosen;
                seats.entry(pid).or_default().push(entry);
            }

            if game.world_era != last_era {
                for pid in majors.iter().copied() {
                    let age = game.players[pid].age.clone();
                    let (score, normal, golden) = before[&pid];
                    let mut catalogue: Vec<(String, i64)> = game
                        .rules
                        .dedications
                        .iter()
                        .filter(|(_, spec)| spec.available_in(game.world_era))
                        .map(|(name, _)| {
                            (name.to_string(), game.projected_dedication_score(pid, name))
                        })
                        .collect();
                    catalogue.sort_by(|left, right| {
                        right.1.cmp(&left.1).then(left.0.cmp(&right.0))
                    });
                    let best = catalogue.first().map_or(0, |entry| entry.1);
                    let offered = catalogue.len();
                    pending.push((
                        pid,
                        Entry {
                            era: game.world_era,
                            turn,
                            age: age.clone(),
                            score,
                            normal,
                            golden,
                            chosen: Vec::new(),
                            rank: offered,
                            offered,
                            best_projection: best,
                            chosen_projection: 0,
                        },
                        catalogue,
                    ));
                    last_age.insert(pid, age);
                }
                last_era = game.world_era;
            }
        }
        // A transition on the last turn still has a choice to read.
        for (pid, mut entry, catalogue) in pending {
            let chosen: Vec<String> = game.players[pid].dedications.iter().cloned().collect();
            let taken = chosen.first().cloned().unwrap_or_default();
            entry.rank = catalogue
                .iter()
                .position(|(name, _): &(String, i64)| *name == taken)
                .unwrap_or(catalogue.len());
            entry.chosen_projection = catalogue
                .iter()
                .find(|(name, _)| *name == taken)
                .map_or(0, |entry| entry.1);
            entry.chosen = chosen;
            seats.entry(pid).or_default().push(entry);
        }

        // Final standing: rank by the engine's own score.
        let mut order: Vec<(usize, i64)> = majors
            .iter()
            .map(|pid| (*pid, game.score(*pid)))
            .collect();
        order.sort_by(|left, right| right.1.cmp(&left.1));
        let seat_count = majors.len();
        majors
            .iter()
            .map(|pid| Seat {
                entries: seats.get(pid).cloned().unwrap_or_default(),
                won: game.winner == Some(*pid),
                rank: order
                    .iter()
                    .position(|(other, _)| other == pid)
                    .unwrap_or(seat_count),
                seats: seat_count,
            })
            .collect::<Vec<Seat>>()
    });

    let seats: Vec<Seat> = per_map.into_iter().flatten().collect();
    let entries: Vec<&Entry> = seats.iter().flat_map(|seat| seat.entries.iter()).collect();
    if entries.is_empty() {
        println!("no age transitions observed — the games never left the opening era");
        return;
    }

    println!("\n== age distribution ==");
    println!("{:>4}  {:>6} {:>6} {:>6} {:>6}   {:>5}", "era", "dark", "normal", "golden", "heroic", "n");
    let mut eras: Vec<usize> = entries.iter().map(|entry| entry.era).collect();
    eras.sort_unstable();
    eras.dedup();
    for era in eras.iter().copied() {
        let rows: Vec<&&Entry> = entries.iter().filter(|entry| entry.era == era).collect();
        let share = |age: &str| {
            let n = rows.iter().filter(|entry| entry.age == age).count();
            format!("{:.0}%", 100.0 * n as f64 / rows.len().max(1) as f64)
        };
        println!(
            "{era:>4}  {:>6} {:>6} {:>6} {:>6}   {:>5}",
            share("dark"),
            share("normal"),
            share("golden"),
            share("heroic"),
            rows.len()
        );
    }
    let total = entries.len();
    let count = |age: &str| entries.iter().filter(|entry| entry.age == age).count();
    println!(
        " all  {:>6} {:>6} {:>6} {:>6}   {:>5}",
        format!("{:.0}%", 100.0 * count("dark") as f64 / total as f64),
        format!("{:.0}%", 100.0 * count("normal") as f64 / total as f64),
        format!("{:.0}%", 100.0 * count("golden") as f64 / total as f64),
        format!("{:.0}%", 100.0 * count("heroic") as f64 / total as f64),
        total
    );

    println!("\n== how close were the misses ==");
    let deficits: Vec<i64> = entries
        .iter()
        .filter(|entry| entry.age == "dark")
        .map(|entry| entry.normal - entry.score)
        .collect();
    let gaps: Vec<i64> = entries
        .iter()
        .filter(|entry| entry.age == "normal")
        .map(|entry| entry.golden - entry.score)
        .collect();
    describe("dark age: Era Score short of Normal", &deficits);
    describe("normal age: Era Score short of Golden", &gaps);

    println!("\n== the Heroic route ==");
    let heroic = count("heroic");
    // A Heroic Age is a Dark Age that reached the *Golden* threshold. Every one
    // is by construction a seat that could also have settled for Normal, so the
    // count is the ceiling on how often a deliberate Dark Age could have paid.
    println!("heroic ages: {heroic} of {total} transitions ({:.1}%)", 100.0 * heroic as f64 / total as f64);
    let after_dark: usize = seats
        .iter()
        .flat_map(|seat| seat.entries.windows(2))
        .filter(|pair| pair[0].age == "dark" && pair[1].age == "heroic")
        .count();
    println!("dark -> heroic in consecutive eras: {after_dark}");

    println!("\n== outcome by age history ==");
    println!("{:>10}  {:>6} {:>8} {:>8}   {:>5}", "held", "win%", "mean rank", "top half", "n");
    for age in ["dark", "normal", "golden", "heroic"] {
        let rows: Vec<&Seat> = seats
            .iter()
            .filter(|seat| seat.entries.iter().any(|entry| entry.age == age))
            .collect();
        if rows.is_empty() {
            continue;
        }
        outcome_row(age, &rows);
    }
    let never_dark: Vec<&Seat> = seats
        .iter()
        .filter(|seat| !seat.entries.is_empty() && seat.entries.iter().all(|entry| entry.age != "dark"))
        .collect();
    outcome_row("never dark", &never_dark);

    // The plain "outcome by age history" table above is confounded and must not
    // be read as causal: a Dark Age is *caused* by a bad era, so of course the
    // seats that had one finish behind. The question "is a Dark Age ever worth
    // having" needs seats that are alike in everything except which side of the
    // threshold they landed on — which is what the margin gives. Within a few
    // points of the line, whether an era ends dark or normal turns on one
    // Eureka or one barbarian camp, and is close to independent of how good the
    // seat is.
    println!("\n== regression discontinuity at the Normal threshold ==");
    println!("(seats alike but for which side of the line they fell)");
    for window in [2i64, 4, 6] {
        let below: Vec<&Seat> = seats
            .iter()
            .filter(|seat| {
                seat.entries
                    .iter()
                    .any(|entry| entry.age == "dark" && entry.score - entry.normal >= -window)
            })
            .collect();
        let above: Vec<&Seat> = seats
            .iter()
            .filter(|seat| {
                seat.entries.iter().any(|entry| {
                    entry.age == "normal"
                        && entry.score - entry.normal < window
                        && entry.score < entry.golden
                })
            })
            .collect();
        println!("\nmargin within {window}:");
        println!("{:>10}  {:>6} {:>8} {:>8}   {:>5}", "held", "win%", "mean rank", "top half", "n");
        outcome_row("just dark", &below);
        outcome_row("just normal", &above);
    }

    // How a Dark Age is supposed to pay: THRESHOLD_SHIFT_PER_PAST_DARK_AGE is
    // -10, so the era after a Dark Age is the cheapest era a civilization will
    // ever have to clear — and clearing it all the way to Golden from a Dark Age
    // is a Heroic Age, three Dedications instead of one.
    println!("\n== what the era after each age looks like ==");
    println!("{:>10}  {:>7} {:>7} {:>7} {:>7}   {:>5}", "after", "dark", "normal", "golden", "heroic", "n");
    for age in ["dark", "normal", "golden", "heroic"] {
        let nexts: Vec<&str> = seats
            .iter()
            .flat_map(|seat| seat.entries.windows(2))
            .filter(|pair| pair[0].age == age)
            .map(|pair| pair[1].age.as_str())
            .collect();
        if nexts.is_empty() {
            continue;
        }
        let share = |want: &str| {
            let n = nexts.iter().filter(|got| **got == want).count();
            format!("{:.0}%", 100.0 * n as f64 / nexts.len() as f64)
        };
        println!(
            "{age:>10}  {:>7} {:>7} {:>7} {:>7}   {:>5}",
            share("dark"),
            share("normal"),
            share("golden"),
            share("heroic"),
            nexts.len()
        );
    }

    println!("\n== dedication choice quality ==");
    let graded: Vec<&&Entry> = entries.iter().filter(|entry| entry.offered > 0).collect();
    let best_taken = graded.iter().filter(|entry| entry.rank == 0).count();
    let regret: i64 = graded
        .iter()
        .map(|entry| entry.best_projection - entry.chosen_projection)
        .sum();
    let informative = graded
        .iter()
        .filter(|entry| entry.best_projection > 0)
        .count();
    println!(
        "took the best-projected dedication: {best_taken} of {} ({:.0}%)",
        graded.len(),
        100.0 * best_taken as f64 / graded.len().max(1) as f64
    );
    println!(
        "transitions where any dedication had a positive projection: {informative} of {}",
        graded.len()
    );
    println!(
        "Era Score left on the table: {regret} total, {:.2} per transition",
        regret as f64 / graded.len().max(1) as f64
    );
    let mut picked: BTreeMap<String, usize> = BTreeMap::new();
    for entry in entries.iter() {
        for name in entry.chosen.iter() {
            *picked.entry(name.clone()).or_insert(0) += 1;
        }
    }
    println!("\nwhich dedication was taken:");
    let mut ordered: Vec<(&String, &usize)> = picked.iter().collect();
    ordered.sort_by(|left, right| right.1.cmp(left.1));
    for (name, n) in ordered {
        println!("  {name:32} {n:>5}");
    }

    println!("\n== era pacing ==");
    let mut lengths: Vec<i64> = Vec::new();
    for seat in seats.iter().take(seats.len().min(1_000)) {
        for pair in seat.entries.windows(2) {
            lengths.push(pair[1].turn as i64 - pair[0].turn as i64);
        }
    }
    describe("turns between age transitions", &lengths);
}

fn outcome_row(label: &str, rows: &[&Seat]) {
    if rows.is_empty() {
        println!("{label:>10}  {:>6} {:>8} {:>8}   {:>5}", "-", "-", "-", 0);
        return;
    }
    let wins = rows.iter().filter(|seat| seat.won).count();
    let mean_rank: f64 =
        rows.iter().map(|seat| seat.rank as f64 + 1.0).sum::<f64>() / rows.len() as f64;
    let top_half = rows
        .iter()
        .filter(|seat| (seat.rank as f64) < seat.seats as f64 / 2.0)
        .count();
    println!(
        "{label:>10}  {:>6} {:>8} {:>8}   {:>5}",
        format!("{:.1}%", 100.0 * wins as f64 / rows.len() as f64),
        format!("{mean_rank:.2}"),
        format!("{:.0}%", 100.0 * top_half as f64 / rows.len() as f64),
        rows.len()
    );
}

fn describe(label: &str, values: &[i64]) {
    if values.is_empty() {
        println!("{label}: no observations");
        return;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let at = |q: f64| sorted[((sorted.len() as f64 - 1.0) * q).round() as usize];
    let mean = sorted.iter().sum::<i64>() as f64 / sorted.len() as f64;
    println!(
        "{label}: n={} mean {:.1}  p10 {}  median {}  p90 {}",
        sorted.len(),
        mean,
        at(0.10),
        at(0.50),
        at(0.90)
    );
}
