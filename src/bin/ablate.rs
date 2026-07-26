//! Oracle ablation: measure the headroom in one subsystem at a time.
//!
//! For a subsystem S, this plays the stock agent against a copy of itself
//! that has been handed a free, cheating version of S, on mirrored maps with
//! seats swapped. The resulting paired win rate is an upper bound on
//! everything any amount of honest work on S could be worth. A grant that
//! wins nothing settles that subsystem for the price of a batch of games
//! rather than the price of a design and a pre-registered run.
//!
//! ```bash
//! cargo run --release --bin ablate -- --grant modernity --pairs 60 --players 4
//! cargo run --release --bin ablate -- --grant all --pairs 40
//! ```
//!
//! `--grant none` is the control and must land at parity; if it does not, the
//! harness is reporting its own noise as headroom.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions};
use civvis::setup::MapSize;
use civvis::oracle::{Grant, Oracle};

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

/// One game. `oracle_seat` is the seat holding the grant; every other major
/// plays the stock agent. Returns whether the granted seat won, and how many
/// times its grant fired.
fn play(options: GameOptions, oracle_seat: usize, grant: Grant) -> (bool, u64) {
    let mut game = Game::new_with(options);
    let mut stock = AdvancedAi::fleet(&game);
    let mut oracle = Oracle::new(AdvancedAi::new(), grant);
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == oracle_seat {
            oracle.take_turn(&mut game, pid);
        } else {
            stock[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    (game.winner == Some(oracle_seat), oracle.fired())
}

/// Wilson score interval, the same statistic the promotion gate uses.
fn wilson(wins: f64, n: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let p = wins / n;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let spread = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((center - spread).max(0.0), (center + spread).min(1.0))
}

/// Exact two-sided sign test over the maps that broke one way or the other.
/// Ties carry no directional information and are excluded, which is what
/// makes this a statement about direction rather than about how many maps
/// happened to be decisive.
fn sign_p(favor: u32, against: u32) -> f64 {
    let n = favor + against;
    if n == 0 {
        return 1.0;
    }
    let mut coefficient = 1.0_f64;
    let mut tail = 0.0_f64;
    let extreme = favor.min(against);
    for k in 0..=n {
        if k > 0 {
            coefficient *= (n - k + 1) as f64 / k as f64;
        }
        if k <= extreme || k >= n - extreme {
            tail += coefficient;
        }
    }
    (tail / 2f64.powi(n as i32)).min(1.0)
}

fn run(grant: Grant, args: &[String]) {
    let pairs = number(args, "--pairs", 40).max(1) as usize;
    let players = number(args, "--players", 4).max(2) as usize;
    let seed = number(args, "--seed", 310_000).max(0) as u64;
    let turns = number(args, "--turns", 500).max(1) as u32;
    let jobs = match number(args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    // The stock map profile for this player count. A map small enough that
    // every army is already next to every city would hide a logistics grant
    // by making logistics free for both sides.
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(args, "--width", default_width as i64).max(8) as i32;
    let height = number(args, "--height", default_height as i64).max(8) as i32;
    let city_states = number(args, "--city-states", size.default_city_states as i64).max(0) as usize;

    // Each map is played twice with the grant on different seats, so a map
    // that simply favours one start cannot be counted as evidence about the
    // grant. A pair is only directional when the two halves disagree.
    let results = civvis::parallel::map(pairs * 2, jobs, |index| {
        let map = index / 2;
        let half = index % 2;
        let options = GameOptions::new(
            players,
            width,
            height,
            seed + map as u64,
            turns,
            city_states,
        );
        let seat = if half == 0 { 0 } else { players - 1 };
        play(options, seat, grant)
    });

    let mut wins = 0u32;
    let mut fired = 0u64;
    let (mut favor, mut against, mut neutral) = (0u32, 0u32, 0u32);
    for map in 0..pairs {
        let (first, first_fired) = results[map * 2];
        let (second, second_fired) = results[map * 2 + 1];
        wins += u32::from(first) + u32::from(second);
        fired += first_fired + second_fired;
        match (first, second) {
            (true, true) => favor += 1,
            (false, false) => against += 1,
            _ => neutral += 1,
        }
    }
    let games = (pairs * 2) as f64;
    let share = wins as f64 / games;
    let (low, high) = wilson(wins as f64, games);
    let p = sign_p(favor, against);
    // The granted seat is one of `players`, so parity is 1/players, not 1/2.
    let parity = 1.0 / players as f64;

    println!("grant {:<10} {pairs} maps, {} games, {players} players, {turns} turns",
        grant.name(), pairs * 2);
    println!("  granted-seat wins   {wins}/{} = {:.1}%  (parity {:.1}%)",
        pairs * 2, 100.0 * share, 100.0 * parity);
    println!("  Wilson 95%          {:.1}%..{:.1}%", 100.0 * low, 100.0 * high);
    println!("  paired direction    for {favor}, against {against}, neutral {neutral}; \
        exact sign p={p:.4}");
    println!("  grant fired         {fired} times ({:.1} per game)", fired as f64 / games);
    if grant != Grant::None && fired == 0 {
        println!("  WARNING: the grant never fired, so this run measured the \
            stock agent under an oracle's name and says nothing about {}",
            grant.name());
    }
    let verdict = if low > parity {
        "HEADROOM — the subsystem limits this agent; work on it can pay"
    } else if high < parity {
        "HARMFUL — free perfection here loses, which means the grant is \
         mis-specified, not that the subsystem is good"
    } else {
        "NO MEASURABLE HEADROOM at this sample size — perfecting this \
         subsystem is worth less than the run can resolve"
    };
    println!("  verdict             {verdict}");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let requested = text(&args, "--grant", "all");
    let grants: Vec<Grant> = if requested == "all" {
        Grant::ALL.to_vec()
    } else {
        match Grant::from_id(&requested) {
            Some(grant) => vec![grant],
            None => {
                eprintln!(
                    "unknown grant {requested:?}; choose from {:?} or all",
                    Grant::ALL.map(Grant::name)
                );
                std::process::exit(2);
            }
        }
    };
    println!(
        "Oracle ablation. Each grant hands one seat a free, cheating version of one\n\
         subsystem and plays it against stock agents on mirrored maps. The result is an\n\
         UPPER BOUND on what honest work on that subsystem could be worth, never a\n\
         playable agent. `none` is the control and must land at parity.\n"
    );
    for grant in grants {
        run(grant, &args);
    }
}
