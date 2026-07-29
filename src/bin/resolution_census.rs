//! Did these games finish, and who finished them?
//!
//! `ablate` scores a game as `game.winner == Some(oracle_seat)`, so a game that
//! reaches the turn limit with **no winner at all** is counted as a loss for the
//! granted seat. The harness never reports how many games resolved, so every
//! win rate it prints bundles "a rival beat this seat" with "nobody won".
//!
//! That is not a nitpick, it is load-bearing right now. The difficulty ladder
//! measured on 2026-07-29 reads:
//!
//! ```text
//! prince 23.0%   king 14.0%   emperor 4.0%   immortal 1.0%   deity 0.0%
//! ```
//!
//! and was written up as *where the agent breaks*. But rising difficulty could
//! produce that same curve two entirely different ways:
//!
//! - **a strength curve** — stronger rivals actually beat the seat, so games
//!   resolve and someone else wins. The reading stands.
//! - **a resolution curve** — the seat is suppressed early, nobody accumulates
//!   a winning position, games run out the clock, and "0.0%" mostly counts
//!   *undecided* games. The reading would be an artifact, and the honest
//!   denominator would be decided games only.
//!
//! `docs/EVAL.md` already records this exact trap for `ai_eval` — *"at 6p/74x46
//! over 250 turns almost nothing resolves, so 50.0% on wins means unmeasured,
//! not equal"* — and `ablate` never inherited the lesson. This binary supplies
//! it: the same cells, the same profile, the same difficulty handling, reporting
//! what `ablate` cannot.
//!
//! ```text
//! cargo run --release --bin resolution_census -- --games 50 --players 4 \
//!     --turns 500 --seed 460000 --difficulty deity
//! ```
//!
//! Cells are built exactly as `ablate` builds them — seats 0 and `players - 1`,
//! `human_seats = {seat}` so only the sampled seat sits on the human side of the
//! handicap — so the numbers are directly comparable to a ladder rung rather
//! than merely similar.
use civvis::ai::Ai;
use civvis::elo::builtin_ai;
use civvis::game::{default_difficulty, Action, Game, GameOptions};
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

/// What one game did.
#[derive(Clone)]
struct Outcome {
    /// `None` when the game reached the turn limit undecided.
    winner: Option<usize>,
    victory: Option<String>,
    turns: u32,
    /// The seat under observation, so "the sampled seat won" can be separated
    /// from "some other major won".
    seat: usize,
}

fn play(options: GameOptions, seat: usize, ai_name: &str) -> Outcome {
    let mut game = Game::new_with(options);
    let mut agents: Vec<Box<dyn Ai>> = game
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
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        agents[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    Outcome {
        winner: game.winner,
        victory: game.victory_type.clone(),
        turns: game.turn,
        seat,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 50).max(1) as usize;
    let players = number(&args, "--players", 4).max(2) as usize;
    let seed = number(&args, "--seed", 460_000).max(0) as u64;
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

    // Exactly `ablate`'s cell construction, so a rung's numbers line up.
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
    println!("{} cells ({games} maps x 2 seats)", cells.len());

    let outcomes = civvis::parallel::map_reporting(
        cells.len(),
        jobs,
        |index| play(options_for(cells[index]), cells[index].1, &ai_name),
        |index, _| {
            if (index + 1) % 25 == 0 {
                println!("  progress {}/{}", index + 1, cells.len())
            }
        },
    );

    let n = outcomes.len() as f64;
    let decided = outcomes.iter().filter(|o| o.winner.is_some()).count();
    let sampled_won = outcomes
        .iter()
        .filter(|o| o.winner == Some(o.seat))
        .count();
    let other_won = decided - sampled_won;
    let undecided = outcomes.len() - decided;
    let mean_turns = outcomes.iter().map(|o| o.turns as f64).sum::<f64>() / n;

    println!("\ndifficulty {difficulty}");
    println!(
        "  DECIDED             {decided}/{} = {:.1}%   undecided {undecided} ({:.1}%)",
        outcomes.len(),
        100.0 * decided as f64 / n,
        100.0 * undecided as f64 / n
    );
    println!("  mean turns          {mean_turns:.0} of {turns}");
    println!(
        "  sampled seat won    {sampled_won}/{} = {:.1}%   (this is ablate's number)",
        outcomes.len(),
        100.0 * sampled_won as f64 / n
    );
    if decided > 0 {
        println!(
            "  ...of DECIDED games {sampled_won}/{decided} = {:.1}%   (the honest denominator)",
            100.0 * sampled_won as f64 / decided as f64
        );
    }
    println!("  another major won   {other_won}");

    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    for o in &outcomes {
        if o.winner.is_some() {
            *kinds
                .entry(o.victory.clone().unwrap_or_else(|| "unrecorded".into()))
                .or_insert(0) += 1;
        }
    }
    if !kinds.is_empty() {
        println!("  victory types among the decided:");
        let mut rows: Vec<(&String, &u64)> = kinds.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in rows {
            println!("    {kind:<22} {count}");
        }
    }

    println!(
        "\nread: if `undecided` is large, `ablate`'s win rate at this difficulty is \
measuring the clock as much as the agent, and the ladder curve should be re-read \
against the decided-games denominator."
    );
}
