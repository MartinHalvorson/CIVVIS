//! When is the top civ decided, and on what?
//!
//! Every seat in these games runs the same `AdvancedAi`, so whatever separates
//! the winner from the field is either the map it was dealt or something the
//! shared logic did differently once conditions diverged. Both readings are
//! actionable and they call for opposite work, so the study measures them
//! apart.
//!
//! For each metric and each sampled turn it reports the **lead-conversion
//! rate**: of the games where a civ led that metric at that turn, how often did
//! that civ go on to finish first. With four seats, chance is 25%. A metric that
//! converts at 30% by turn 40 is where the game is already being decided.
//!
//! ⚠ **`start_yield` is the control and must be read first.** It is the food
//! and production on the tiles around each capital at turn 1 — pure map, before
//! the agent has done anything. Whatever it converts at is the share of
//! "winner behaviour" that is really the deal. A behavioural metric only earns
//! attention where it converts *above* the start.
//!
//! Two of the rows are behaviour rather than stock. **`steadiness`** is the
//! number of grand-strategy switches so far, negated, so that leading it means
//! having switched the fewest times; **`held_course`** counts only the switches
//! that crossed none of the three board conditions a switch can answer to (war,
//! a threatened city, a city deficit). Reading them together separates two
//! different claims: if `steadiness` converts above chance the planner is
//! simply churning, and if only `held_course` does, the churn that costs games
//! is specifically the kind nothing on the board asked for.
//!
//! ⚠ Both are **correlates, and the causal arrow is genuinely ambiguous** — a
//! civ that is losing gets attacked, flips to Recovery, and churns, so late
//! samples partly measure the outcome rather than predict it. This is why the
//! early columns matter more than the late ones and why the two map controls
//! sit at the top of the same table.
//!
//! ⚠ **Defaults are the deployment shape** — 6 players, 74x46, 9 city-states,
//! Online, 250 turns — and that is deliberate. The first run of this study used
//! 4p 60x38 at Standard, and on 2026-08-10 a change gated at `ai_eval`'s small
//! defaults measured **+20 Elo there and parity at deployment**, with the sign
//! flipping outright once the profile moved. A correlate read on the wrong
//! board is no safer than an effect measured on one.
//!
//! Usage: leader_study [--games N] [--start-seed N] [--players N] [--turns N]
//!                     [--width N] [--height N] [--city-states N] [--speed ID]
//!                     [--jobs N]
use std::collections::BTreeMap;

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions};

/// The three conditions a grand-strategy switch can legitimately answer to.
///
/// Taken verbatim from `ai_eval`'s boundary test so the two tools cannot drift
/// into disagreeing about what "unanchored" means: a switch that crosses none
/// of these did not answer to anything the board did.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Anchor {
    at_major_war: bool,
    threatened: bool,
    city_deficit: bool,
}

/// This seat's strategy and the board conditions behind it, or `None` when the
/// agent reports no plan at all.
fn posture(g: &Game, pid: usize, ai: &dyn Ai) -> Option<(&'static str, Anchor)> {
    let at_major_war = g.players.iter().any(|player| {
        player.id != pid
            && player.alive
            && !player.is_minor
            && !player.is_barbarian
            && g.is_at_war(pid, player.id)
    });
    ai.plan_report().map(|plan| {
        (
            plan.strategy,
            Anchor {
                at_major_war,
                threatened: plan.threatened_city.is_some(),
                city_deficit: g.player_city_ids(pid).len() < plan.desired_cities,
            },
        )
    })
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Turns at which every civ is measured. Dense early, because that is where a
/// lever would still have time to pay.
const SAMPLES: [u32; 9] = [1, 20, 40, 60, 80, 100, 130, 160, 200];

const METRICS: [&str; 13] = [
    "start_yield",
    "start_room",
    "steadiness",
    "held_course",
    "cities",
    "pop",
    "techs",
    "civics",
    "districts",
    "holy_sites",
    "faith",
    "gold",
    "military",
];

/// How far from a capital a tile still counts toward the start it was dealt.
const START_RADIUS: i32 = 3;

/// How far out "room to expand" reaches. Wider than `START_RADIUS`, because the
/// question is where the second and third city go, not what the first works.
const ROOM_RADIUS: i32 = 6;

fn measure(g: &Game, pid: usize, start: (f64, f64), churn: (f64, f64)) -> Vec<f64> {
    let cities = g.player_city_ids(pid);
    let pop: i64 = cities.iter().map(|cid| i64::from(g.cities[cid].pop)).sum();
    let districts: usize = cities.iter().map(|cid| g.cities[cid].districts.len()).sum();
    let holy: usize = cities
        .iter()
        .filter(|cid| g.city_has_district_family(&g.cities[cid], civvis::name!("holy_site")))
        .count();
    let p = &g.players[pid];
    vec![
        start.0,
        start.1,
        // Negated, so that "leading" this metric means the FEWEST switches and
        // the row reads the same way as every other: above chance means the
        // trait predicts finishing first.
        -churn.0,
        -churn.1,
        cities.len() as f64,
        pop as f64,
        p.techs.len() as f64,
        p.civics.len() as f64,
        districts as f64,
        holy as f64,
        p.faith,
        p.gold,
        g.military_power(pid),
    ]
}

/// Food plus production on the tiles around this civ's capital.
///
/// Frozen the first time the civ has one, which is the site it was dealt and
/// chose rather than anything it has since built -- the control for "the winner
/// simply had the better ground".
fn start_yield(g: &Game, pid: usize) -> (f64, f64) {
    let Some(cid) = g.player_city_ids(pid).into_iter().next() else {
        return (0.0, 0.0);
    };
    let home = g.cities[&cid].pos;
    let mut yield_near = 0.0;
    let mut room = 0.0;
    for (pos, tile) in g.map.tiles.iter() {
        let d = g.wdist(home, *pos);
        if d <= START_RADIUS {
            let y = g.rules.tile_yields(tile);
            yield_near += y.food + y.production;
        }
        // Dry, unclaimed ground inside settling range: the room this civ has to
        // put its second and third city, which `start_yield` does not capture
        // and which would otherwise confound the `cities` row.
        if d <= ROOM_RADIUS && !g.rules.is_water(tile) && tile.owner_city.is_none() {
            room += 1.0;
        }
    }
    (yield_near, room)
}

/// One game's samples: `[sample][civ][metric]`, plus the final ranking.
struct Trace {
    samples: Vec<Vec<Vec<f64>>>,
    finish: Vec<usize>,
}

/// The competing civilizations, and only those.
///
/// ⚠ `g.players` is NOT the table. A 4-player game carries twelve players:
/// four majors, six city-states, and two barbarian seats. Measuring all of them
/// silently ranks city-states as rivals — they hold a founded city on turn 1
/// when no major does, so they lead every early metric and never win, which is
/// how the first run of this study produced a control row reading exactly 0% at
/// every turn. That impossible number is the only reason the bug was caught.
fn majors(g: &Game) -> Vec<usize> {
    g.players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect()
}

fn play(options: GameOptions) -> Trace {
    let mut g = Game::new_with(options);
    let seats = majors(&g);
    let players = g.players.len();
    let mut ais = AdvancedAi::fleet(&g);
    let mut samples: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut starts = vec![(0.0, 0.0); players];
    // Per seat: switches so far, and the subset that crossed no board
    // condition. Counted as the game runs, because a plan is a live object --
    // nothing at the end of the game records how many times it changed.
    let mut churn = vec![(0.0f64, 0.0f64); players];
    let mut last: Vec<Option<(&'static str, Anchor)>> = vec![None; players];
    let mut next = 0usize;
    while g.winner.is_none() {
        while next < SAMPLES.len() && g.turn >= SAMPLES[next] {
            // Freeze each civ's start the first time it actually has a
            // capital. Turn 1 is too early -- no major has founded yet, and a
            // control that is zero for everyone silently measures nothing.
            for pid in &seats {
                if starts[*pid].0 == 0.0 {
                    starts[*pid] = start_yield(&g, *pid);
                }
            }
            samples.push(
                seats
                    .iter()
                    .map(|pid| measure(&g, *pid, starts[*pid], churn[*pid]))
                    .collect(),
            );
            next += 1;
        }
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        // Read the plan the seat just acted on, not the one it will hold next
        // turn. A switch is counted once, on the turn the strategy changed.
        if let Some(now) = posture(&g, pid, &ais[pid]) {
            if let Some(before) = last[pid] {
                if before.0 != now.0 {
                    churn[pid].0 += 1.0;
                    if before.1 == now.1 {
                        churn[pid].1 += 1.0;
                    }
                }
            }
            last[pid] = Some(now);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    // Rank by the engine's own tiebreak, winner first if there is one. Indices
    // are positions in `seats`, matching the sample rows.
    let mut finish: Vec<usize> = (0..seats.len()).collect();
    finish.sort_by(|a, b| {
        g.score_rank_key(seats[*b])
            .cmp(&g.score_rank_key(seats[*a]))
    });
    if let Some(w) = g.winner {
        if let Some(slot) = seats.iter().position(|pid| *pid == w) {
            finish.retain(|i| *i != slot);
            finish.insert(0, slot);
        }
    }
    Trace { samples, finish }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 40).max(1) as u64;
    let start = number(&args, "--start-seed", 770_000) as u64;
    let players = number(&args, "--players", 6).max(2) as usize;
    let turns = number(&args, "--turns", 250).max(1) as u32;
    let width = number(&args, "--width", 74) as i32;
    let height = number(&args, "--height", 46) as i32;
    let city_states = number(&args, "--city-states", 9) as usize;
    let speed = text(&args, "--speed", "online");
    let jobs = number(&args, "--jobs", civvis::parallel::default_jobs() as i64).max(1) as usize;

    let traces = civvis::parallel::map(games as usize, jobs, |offset| {
        let seed = start + offset as u64;
        let mut options = GameOptions::new(players, width, height, seed, turns, city_states);
        options.speed = speed.clone();
        play(options)
    });

    let chance = 100.0 / players as f64;
    println!(
        "leader study: {games} games x {players}p {width}x{height}, {city_states} city-states, \
         {speed}, {turns} turns, seeds {start}..{}",
        start + games - 1
    );
    println!(
        "lead-conversion: of games where a civ led this metric at this turn, \
         how often it finished first. chance = {chance:.0}%.\n\
         ⚠ start_yield and start_room are the MAP, not behaviour — read every row\n         against them. start_room is the unclaimed dry ground in settling range,\n         the control for the cities row.\n"
    );

    print!("{:<13}", "metric");
    for turn in SAMPLES {
        print!("{:>7}", format!("t{turn}"));
    }
    println!();

    // `wins[metric][sample] = (converted, decided)`, skipping ties so a metric
    // is never credited for a lead nobody held.
    let mut tally: BTreeMap<(usize, usize), (usize, usize)> = BTreeMap::new();
    for trace in traces.iter() {
        let champion = trace.finish[0];
        for (s, sample) in trace.samples.iter().enumerate() {
            for m in 0..METRICS.len() {
                let best = sample
                    .iter()
                    .map(|civ| civ[m])
                    .fold(f64::NEG_INFINITY, f64::max);
                let leaders: Vec<usize> = (0..sample.len())
                    .filter(|pid| (sample[*pid][m] - best).abs() < 1e-9)
                    .collect();
                if leaders.len() != 1 {
                    continue; // a shared lead is not a lead
                }
                let entry = tally.entry((m, s)).or_insert((0, 0));
                entry.1 += 1;
                if leaders[0] == champion {
                    entry.0 += 1;
                }
            }
        }
    }

    for (m, name) in METRICS.iter().enumerate() {
        print!("{name:<13}");
        for s in 0..SAMPLES.len() {
            match tally.get(&(m, s)) {
                Some((hit, seen)) if *seen >= 5 => {
                    print!(
                        "{:>7}",
                        format!("{:.0}%", 100.0 * *hit as f64 / *seen as f64)
                    )
                }
                _ => print!("{:>7}", "-"),
            }
        }
        println!();
    }

    println!(
        "\n'-' means fewer than five games had an outright leader on that metric \
         at that turn."
    );

    // A second reading, because the first one throws away too much on any
    // metric that is a small integer. `steadiness` and `held_course` count
    // switches, so two seats share the lead constantly and the outright-leader
    // test skips the game entirely -- on a six-game trial `held_course` had no
    // readable column at all. Mean rank uses every game: rank the champion
    // among its rivals on the metric, averaging tied ranks, and average that
    // over games. Chance is the middle rank; BELOW chance means the eventual
    // champion was already ahead on this metric at that turn.
    let middle = (players as f64 + 1.0) / 2.0;
    println!(
        "\nchampion's mean rank on each metric (1 = best of {players}). \
         chance = {middle:.2}; lower is earlier separation."
    );
    print!("{:<13}", "metric");
    for turn in SAMPLES {
        print!("{:>7}", format!("t{turn}"));
    }
    println!();

    let mut ranks: BTreeMap<(usize, usize), (f64, usize)> = BTreeMap::new();
    for trace in traces.iter() {
        let champion = trace.finish[0];
        for (s, sample) in trace.samples.iter().enumerate() {
            for m in 0..METRICS.len() {
                let mine = sample[champion][m];
                let better = sample.iter().filter(|civ| civ[m] > mine).count();
                let tied = sample
                    .iter()
                    .filter(|civ| (civ[m] - mine).abs() < 1e-9)
                    .count();
                // Average the block of tied ranks rather than picking an end of
                // it, so a metric on which everyone is equal scores exactly at
                // chance instead of looking like a lead or a deficit.
                let rank = better as f64 + (tied as f64 + 1.0) / 2.0;
                let entry = ranks.entry((m, s)).or_insert((0.0, 0));
                entry.0 += rank;
                entry.1 += 1;
            }
        }
    }
    for (m, name) in METRICS.iter().enumerate() {
        print!("{name:<13}");
        for s in 0..SAMPLES.len() {
            match ranks.get(&(m, s)) {
                Some((total, seen)) if *seen >= 5 => {
                    print!("{:>7}", format!("{:.2}", *total / *seen as f64))
                }
                _ => print!("{:>7}", "-"),
            }
        }
        println!();
    }
}
