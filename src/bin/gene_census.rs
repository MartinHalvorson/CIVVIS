//! Which of the forty genes actually reach a decision the deployed agent makes.
//!
//! `evolve` breeds a 40-gene `Weights` vector and `league` rates the survivors,
//! so the whole selection pipeline assumes every gene is a live control surface.
//! Nothing had checked that. Two genes measured on 2026-08-10 disagree sharply:
//! `d_holy` moved 20 Elo, while `settle_food` at the roster winners' value left
//! **398 of 400 paired maps outcome-identical** and 390 of 400 terminal scores
//! byte-equal. A gene that cannot change an outcome cannot be selected on, and
//! breeding on it is a proxy for nothing.
//!
//! This is not an Elo harness. It answers the prior question — *does this gene
//! change anything at all* — which is far cheaper to establish than a win rate
//! and is a precondition for the win rate meaning anything. For each gene it
//! plays paired games where one seat carries the perturbed genome and the same
//! seat carries the stock genome in the control, on identical seeds, and reports
//! the fraction of games whose outcome is untouched.
//!
//! A gene reported at 100% identical is inert **at this profile**, which is a
//! claim about the deployed decision path, not about the rule the gene names.
//!
//! ⚠ **A single low-`--games` pass is a screen, not a result.** Twelve games
//! bound a gene's per-game effect rate at roughly 25% and no tighter, and the
//! first pass duly called `faith_builder` inert when 48 games put it at 12%.
//! Re-probe anything this reports INERT at a higher `--games` on disjoint
//! seeds before believing it; the eight that survived that treatment are
//! recorded in `docs/EVAL.md` 2026-08-10.
//!
//! Usage: gene_census [--games N] [--start-seed N] [--players N] [--turns N]
//!                    [--width N] [--height N] [--city-states N] [--gene NAME] [--jobs N]
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game, GameOptions};

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

/// What one game came to, in the terms a selection pass can actually read.
#[derive(PartialEq, Debug)]
struct Outcome {
    winner: Option<usize>,
    victory: String,
    turn: u32,
    scores: Vec<i64>,
}

fn play(options: GameOptions, treated_seat: usize, weights: &Weights) -> Outcome {
    let mut g = Game::new_with(options);
    let mut ais: Vec<Box<dyn Ai>> = (0..g.players.len())
        .map(|pid| {
            if pid == treated_seat {
                Box::new(AdvancedAi::with_weights(weights.clone())) as Box<dyn Ai>
            } else {
                Box::new(AdvancedAi::new()) as Box<dyn Ai>
            }
        })
        .collect();
    while g.winner.is_none() {
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    let scores = (0..g.players.len()).map(|pid| g.score(pid)).collect();
    Outcome {
        winner: g.winner,
        victory: g.victory_type.clone().unwrap_or_default(),
        turn: g.reported_turn(),
        scores,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 12).max(1) as u64;
    let start = number(&args, "--start-seed", 990_000) as u64;
    let players = number(&args, "--players", 4).max(2) as usize;
    let turns = number(&args, "--turns", 220).max(1) as u32;
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let city_states = number(&args, "--city-states", 6) as usize;
    let only = text(&args, "--gene");
    let jobs = number(&args, "--jobs", civvis::parallel::default_jobs() as i64).max(1) as usize;

    let names = Weights::gene_names();
    let bounds = Weights::bounds();
    let stock = Weights::advanced();
    let stock_vec = stock.to_vec();

    println!(
        "gene census: {games} games x {players}p {width}x{height}, {turns} turns, \
         seeds {start}..{}",
        start + games - 1
    );
    println!(
        "perturbing one seat's genome against a stock fleet; \
         'identical' = same winner, victory, end turn, and every score\n"
    );
    println!(
        "{:<20} {:>8} {:>8} {:>10}  verdict",
        "gene", "stock", "probe", "identical"
    );

    let mut inert = Vec::new();
    let mut live = Vec::new();

    for (index, name) in names.iter().enumerate() {
        if only.as_deref().is_some_and(|want| want != *name) {
            continue;
        }
        // Probe at the far end of the gene's own legal range, so a gene
        // reported inert is inert against the widest move the GA could ever
        // breed -- not merely against a timid nudge.
        let (lo, hi) = bounds[index];
        let current = stock_vec[index];
        let probe = if (current - lo).abs() > (hi - current).abs() {
            lo
        } else {
            hi
        };
        let mut probe_vec = stock_vec.clone();
        probe_vec[index] = probe;
        let probed = Weights::from_vec(&probe_vec);

        // Each seed is an independent pair, so the batch is embarrassingly
        // parallel; a game does not fit a thread's stock stack, which is why
        // this goes through the repository's pool rather than raw threads.
        let matches = civvis::parallel::map(games as usize, jobs, |offset| {
            let seed = start + offset as u64;
            // Rotate the treated seat so the finding is not one seat's quirk.
            let treated = (seed as usize) % players;
            let options = || GameOptions::new(players, width, height, seed, turns, city_states);
            play(options(), treated, &stock) == play(options(), treated, &probed)
        });
        let identical = matches.iter().filter(|same| **same).count();

        let share = identical as f64 / games as f64;
        let verdict = if identical == games as usize {
            inert.push(*name);
            "INERT — no game moved"
        } else if share >= 0.9 {
            "nearly inert"
        } else {
            live.push(*name);
            "live"
        };
        println!(
            "{name:<20} {current:>8.3} {probe:>8.3} {:>9.0}%  {verdict}",
            share * 100.0
        );
    }

    if only.is_none() {
        println!(
            "\n{} of {} genes never changed a single game outcome at this profile.",
            inert.len(),
            names.len()
        );
        if !inert.is_empty() {
            println!("inert: {}", inert.join(", "));
        }
        println!("live: {}", live.join(", "));
        println!(
            "\nA gene that cannot move an outcome cannot be selected on. Genes listed \
             inert are breeding surface that `evolve` and `league` spend their budget \
             on for no return, and any rating fitted over them is fitted over noise \
             in those coordinates."
        );
    }
}
