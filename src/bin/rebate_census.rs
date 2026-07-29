//! What does an empire buy with a Settler's price when it is short of cities?
//!
//! `Grant::Expansion` is the only grant in `oracle.rs` that has ever returned
//! headroom — 23.0% to 52.3% over 400 maps — and a dozen pull requests on the
//! expansion pipeline rest on it. `Grant::Rebate` is its cost-matched control:
//! same firing schedule, same city, same price, no Settler.
//!
//! But a *win* for the rebate would be ambiguous on its own. The empire might
//! have turned the money into cities anyway, in which case the rebate has
//! simply reproduced the expansion grant honestly; or it might have spent it
//! on something else entirely, in which case a win says the headroom was never
//! about cities. `ablate` reports only whether the granted seat won and how
//! often the grant fired, so it cannot tell those apart.
//!
//! This census can. It records, for each payment:
//!
//! - what the payout city had at the head of its queue when the money landed,
//! - and, at the end of the game, how many cities the seat held and how many
//!   Settlers it ever trained,
//!
//! against a matched control seat on the same map with no grant at all, and
//! against the expansion grant on the same cells. Three arms, one map set.
//!
//! ```text
//! cargo run --release --bin rebate_census -- --games 12 --players 4 \
//!     --turns 500 --seed 470000
//! ```
//!
//! This is a census, not an evaluation. It says what the money bought, never
//! whether buying it was good — twelve games cannot resolve a win rate and no
//! number printed here should be read as one.
use civvis::ai::Ai;
use civvis::elo::builtin_ai;
use civvis::game::{default_difficulty, Action, Game, GameOptions, Item};
use civvis::oracle::{expansion_payout_city, Grant, Oracle};
use civvis::rules::Rules;
use civvis::setup::MapSize;
use std::collections::{BTreeMap, BTreeSet};

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

/// What the payout city was building when a payment landed, coarsened to the
/// distinction the question turns on: a Settler, or anything else.
fn queue_head_label(g: &Game, cid: u32) -> &'static str {
    match g.cities.get(&cid).and_then(|city| city.queue.first()) {
        None => "empty queue",
        Some(Item::Unit { unit }) if unit == "settler" => "a settler",
        Some(Item::Unit { .. }) | Some(Item::Formation { .. }) => "another unit",
        Some(Item::Building { .. }) => "a building",
        Some(Item::District { .. }) => "a district",
        Some(Item::Wonder { .. }) => "a wonder",
        Some(_) => "a project or repair",
    }
}

/// What one game's granted seat did.
#[derive(Default, Clone)]
struct Census {
    /// Times the grant's payout condition held at the top of the seat's turn.
    payments: u64,
    /// Queue head at each of those moments.
    heads: BTreeMap<String, u64>,
    cities_at_end: usize,
    settlers_trained: i64,
    peak_cities: usize,
    won: bool,
}

/// One game, wrapping `oracle_seat` in `grant` and reading the seat's expansion
/// behaviour off the final position.
///
/// The queue-head observation is taken at the top of the seat's turn *before*
/// the wrapped agent plays, which is exactly when `Oracle::take_turn` applies
/// the grant, so the head recorded is the item the payment actually landed on.
/// It is read for every arm including the control, so the three histograms are
/// comparable rather than being a property of the grant that produced them.
fn play(options: GameOptions, oracle_seat: usize, grant: Grant, ai_name: &str) -> Census {
    let mut game = Game::new_with(options);
    let mut stock: Vec<Box<dyn Ai>> = game
        .players
        .iter()
        .map(|player| {
            let name = if player.is_minor || player.is_barbarian {
                "basic"
            } else {
                ai_name
            };
            builtin_ai(name, game.seed.wrapping_add(player.id as u64))
        })
        .collect();
    let mut oracle = Oracle::new(
        builtin_ai(ai_name, game.seed.wrapping_add(oracle_seat as u64)),
        grant,
    );
    let mut census = Census::default();
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == oracle_seat {
            if let Some(home) = expansion_payout_city(&game, pid) {
                census.payments += 1;
                *census
                    .heads
                    .entry(queue_head_label(&game, home).to_string())
                    .or_insert(0) += 1;
            }
            oracle.take_turn(&mut game, pid);
        } else {
            stock[pid].take_turn(&mut game, pid);
        }
        census.peak_cities = census.peak_cities.max(game.player_city_ids(oracle_seat).len());
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    census.cities_at_end = game.player_city_ids(oracle_seat).len();
    census.settlers_trained = game.players[oracle_seat]
        .counters
        .get("trained:settler")
        .copied()
        .unwrap_or(0);
    census.won = game.winner == Some(oracle_seat);
    census
}

fn report(label: &str, runs: &[Census], games: f64) {
    let payments: u64 = runs.iter().map(|run| run.payments).sum();
    let cities: usize = runs.iter().map(|run| run.cities_at_end).sum();
    let peak: usize = runs.iter().map(|run| run.peak_cities).sum();
    let settlers: i64 = runs.iter().map(|run| run.settlers_trained).sum();
    let wins = runs.iter().filter(|run| run.won).count();
    println!("\ngrant {label}");
    println!(
        "  cities at end       {:.2}   peak {:.2}   settlers trained {:.2}",
        cities as f64 / games,
        peak as f64 / games,
        settlers as f64 / games
    );
    println!(
        "  payout turns        {payments} ({:.1} per game)   won {wins}/{} (not a win rate)",
        payments as f64 / games,
        runs.len()
    );
    let mut heads: BTreeMap<String, u64> = BTreeMap::new();
    for run in runs {
        for (head, count) in &run.heads {
            *heads.entry(head.clone()).or_insert(0) += count;
        }
    }
    if payments > 0 {
        println!("  what it was building when the money would land:");
        let mut rows: Vec<(&String, &u64)> = heads.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1));
        for (head, count) in rows {
            println!(
                "    {head:<22} {count:>6}  {:>5.1}%",
                100.0 * *count as f64 / payments as f64
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 12).max(1) as usize;
    let players = number(&args, "--players", 4).max(2) as usize;
    let seed = number(&args, "--seed", 470_000).max(0) as u64;
    let turns = number(&args, "--turns", 500).max(1) as u32;
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(&args, "--width", default_width as i64).max(8) as i32;
    let height = number(&args, "--height", default_height as i64).max(8) as i32;
    let city_states =
        number(&args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let speed = text(&args, "--speed", &civvis::game::default_speed());
    let ai_name = text(&args, "--ai", "advanced");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }

    // The same (map, seat) cells for all three arms, so the three histograms
    // and the three city counts describe the same starts.
    let cells: Vec<(usize, usize)> = (0..games)
        .flat_map(|map| [0usize, players - 1].into_iter().map(move |seat| (map, seat)))
        .collect();
    let options_for = |cell: (usize, usize)| {
        let mut options = GameOptions::new(
            players,
            width,
            height,
            seed + cell.0 as u64,
            turns,
            city_states,
        );
        options.difficulty = difficulty.clone();
        options.speed = speed.clone();
        options.human_seats = BTreeSet::from([cell.1]);
        options
    };

    println!(
        "profile: {players}p {width}x{height}, {city_states} city-states, \
{turns} {speed} turns, seed {seed}, {jobs} jobs, difficulty {difficulty}, {ai_name}"
    );
    println!(
        "{} cells ({games} maps x 2 seats) played under each of none, rebate, expansion",
        cells.len()
    );
    println!("this is a census of what the money bought, not an evaluation of whether it helped");

    let arm_games = cells.len() as f64;
    for grant in [Grant::None, Grant::Rebate, Grant::Expansion] {
        let runs: Vec<Census> = civvis::parallel::map_reporting(
            cells.len(),
            jobs,
            |index| play(options_for(cells[index]), cells[index].1, grant, &ai_name),
            |index, _| println!("  {} progress {}/{}", grant.name(), index + 1, cells.len()),
        );
        report(grant.name(), &runs, arm_games);
    }

    println!(
        "\nread: if `rebate` ends on the control's city count, the rebate did not buy \
cities and a rebate win in `ablate` would be generic economy. If it ends near \
`expansion`'s, the rebate bought cities honestly and the two grants are measuring \
the same thing by different routes."
    );
}
