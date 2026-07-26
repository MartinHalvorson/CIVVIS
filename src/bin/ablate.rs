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

/// Exact two-sided binomial tail for `hits` of `n` at p=1/2.
///
/// Used for McNemar's test over discordant pairs: under the null that the
/// grant changes nothing, a pair that disagrees is equally likely to
/// disagree either way.
fn exact_two_sided(hits: u32, n: u32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut coefficient = 1.0_f64;
    let mut tail = 0.0_f64;
    let extreme = hits.min(n - hits);
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

/// One (map, seat) cell: the same game played with the grant and without it.
#[derive(Clone, Copy)]
struct Cell {
    map: usize,
    seat: usize,
}

fn run(grants: &[Grant], args: &[String]) {
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
    let city_states =
        number(args, "--city-states", size.default_city_states as i64).max(0) as usize;

    // Every map is played from two different seats so a map that simply
    // favours one start cannot be read as evidence about a grant.
    let cells: Vec<Cell> = (0..pairs)
        .flat_map(|map| {
            [0usize, players - 1]
                .into_iter()
                .map(move |seat| Cell { map, seat })
        })
        .collect();

    let options_for = |cell: Cell| {
        GameOptions::new(players, width, height, seed + cell.map as u64, turns, city_states)
    };

    // The control is played once and shared by every grant. Each grant is then
    // compared against it cell by cell — same map, same seat, same seed — so
    // the comparison is matched and map variance drops out instead of being
    // averaged over. Comparing a granted seat's raw win rate against 1/players
    // instead would have to carry all of that variance, which at these sample
    // sizes is most of the signal.
    println!("playing {} control games...", cells.len());
    let control: Vec<bool> = civvis::parallel::map(cells.len(), jobs, |index| {
        play(options_for(cells[index]), cells[index].seat, Grant::None).0
    });
    let control_wins = control.iter().filter(|won| **won).count();
    println!(
        "control: granted seat won {control_wins}/{} = {:.1}% (parity {:.1}%)\n",
        cells.len(),
        100.0 * control_wins as f64 / cells.len() as f64,
        100.0 / players as f64
    );

    for &grant in grants {
        let played = civvis::parallel::map(cells.len(), jobs, |index| {
            play(options_for(cells[index]), cells[index].seat, grant)
        });
        let treated: Vec<bool> = played.iter().map(|(won, _)| *won).collect();
        let fired: u64 = played.iter().map(|(_, fired)| *fired).sum();

        let wins = treated.iter().filter(|won| **won).count();
        // McNemar: only the cells where the grant changed the outcome carry
        // information about the grant.
        let mut helped = 0u32;
        let mut hurt = 0u32;
        for (with, without) in treated.iter().zip(&control) {
            match (with, without) {
                (true, false) => helped += 1,
                (false, true) => hurt += 1,
                _ => {}
            }
        }
        let discordant = helped + hurt;
        let p = exact_two_sided(helped, discordant);
        let n = cells.len() as f64;

        println!("grant {:<10} {pairs} maps x 2 seats, {players} players, {turns} turns",
            grant.name());
        println!("  granted seat won    {wins}/{} = {:.1}%   (control {control_wins} = {:.1}%)",
            cells.len(), 100.0 * wins as f64 / n, 100.0 * control_wins as f64 / n);
        println!("  matched pairs       grant won where control lost: {helped}; \
lost where control won: {hurt}; unchanged: {}", cells.len() as u32 - discordant);
        println!("  McNemar exact       p={p:.4} over {discordant} discordant cells");
        println!("  grant fired         {fired} times ({:.1} per game)", fired as f64 / n);
        if grant != Grant::None && fired == 0 {
            println!("  WARNING: the grant never fired, so this measured the stock \
agent under an oracle's name and says nothing about {}", grant.name());
        }
        let verdict = if grant == Grant::None {
            if discordant == 0 {
                "SANITY OK — the null grant reproduced the control exactly, so \
the harness is deterministic and adds nothing of its own"
            } else {
                "BROKEN — the null grant changed outcomes, so every number \
here includes harness noise and none of it can be trusted"
            }
        } else if discordant < 8 {
            "TOO FEW DISCORDANT CELLS to say anything — raise --pairs"
        } else if p >= 0.05 {
            "NO MEASURABLE HEADROOM — perfecting this subsystem is worth less \
than this run can resolve"
        } else if helped > hurt {
            "HEADROOM — this subsystem limits the agent; work on it can pay"
        } else {
            "HARMFUL — free perfection here loses, so the grant is \
mis-specified rather than the subsystem being fine"
        };
        println!("  verdict             {verdict}");
        println!();
    }
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
    run(&grants, &args);
}
