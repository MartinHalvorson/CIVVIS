//! How much more macro search is worth buying, measured on the validated fitness.
//!
//! Everything in `docs/GENOME.md` says the genome is a dead end: eleven of
//! forty-eight genes cannot change a game, and on the statistic that tracks
//! winning **not one of the eight blocks is load-bearing**. Meanwhile every
//! promoted gain in this repository came from the same place — more
//! counterfactual rollout. `strategic_deep` (review every 20 turns, horizon 80)
//! is +45 Elo over `strategic` and is the shipped agent.
//!
//! So the question worth compute is not which weights to breed. It is **how
//! much further that one working lever goes**, and the repository's answer is
//! unresolved for a specific and expensive reason: those doses were decided on
//! win rates, which needed 120–300 mirrored maps each, and several arms still
//! came back `INCONCLUSIVE`.
//!
//! Victory-lane progress changes that arithmetic. It is the only statistic
//! measured here that reports parity on a change whose wins answer is parity
//! and a clear positive on one whose wins answer is positive, and it carries
//! roughly a **10× variance advantage over a binary win rate** — SE 0.0146 on
//! 60 map-pairs against 0.0456 on 120 games. Doses that cost 300 maps to
//! resolve on wins are affordable on this.
//!
//! ```text
//! search_dose --maps 24
//! ```
//!
//! The control is the promoted `strategic_deep`, not the old baseline, so every
//! reading answers "is this worth buying **on top of what already shipped**".
//!
//! Two things the repository already knows, which this is designed to respect:
//!
//! - **The horizon saturates.** A branch that reaches a decided game returns
//!   exactly 1.0 or 0.0, so once every branch resolves they agree by
//!   construction: 22% of reviews are in that state at horizon 40, 56% at 80,
//!   89% at 120. Depth past 80 buys agreement, not discrimination — so a null
//!   at horizon 120 is the *predicted* result and confirms the instrument
//!   rather than the idea.
//! - **Quadrupling reviews is not four times better.** `strategic_r10` was the
//!   weakest of the three doublings. Frequency and depth are not
//!   interchangeable, and the point of a grid is to see which axis still pays.
//!
//! Nothing here is a promotion. A dose that reads positive on lane progress
//! earns a pre-registered run on **wins**, which is what decides it.
use civvis::ai::{Ai, AdvancedAi, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::strategic::StrategicAi;

/// (label, review_every, horizon). The control is the promoted configuration.
const CONTROL: (&str, u32, u32) = ("deep 20/80 (shipped)", 20, 80);
const DOSES: [(&str, u32, u32); 5] = [
    ("20/120  deeper", 20, 120),
    ("10/80   2x reviews", 10, 80),
    ("10/120  both", 10, 120),
    ("40/80   half reviews", 40, 80),
    ("20/40   half horizon", 20, 40),
];

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn agent(review_every: u32, horizon: u32) -> StrategicAi {
    let mut ai = StrategicAi::with_weights(Weights::default());
    ai.review_every = review_every;
    ai.horizon = horizon;
    ai
}

/// One mirrored map pair: the dose against the control, scored on the treated
/// seats' share of victory-lane progress.
fn duel(
    dose: (u32, u32),
    players: usize,
    w: i32,
    h: i32,
    seed: u64,
    turns: u32,
) -> f64 {
    let mut share = 0.0;
    for direction in 0..2usize {
        let mut game = Game::new(players, w, h, seed, turns, 0);
        let is_treated = |pid: usize| pid % 2 == direction;
        // The macro search is expensive, so only the seats under test carry it;
        // the rest of the table is the stock fleet, identical in both arms.
        let mut treatment: Vec<StrategicAi> =
            (0..game.players.len()).map(|_| agent(dose.0, dose.1)).collect();
        let mut control: Vec<StrategicAi> = (0..game.players.len())
            .map(|_| agent(CONTROL.1, CONTROL.2))
            .collect();
        let mut minors: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if game.players[pid].is_minor {
                    minors[pid].take_turn(&mut game, pid);
                } else if is_treated(pid) {
                    treatment[pid].take_turn(&mut game, pid);
                } else {
                    control[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        let mut mine = 0.0;
        let mut table = 0.0;
        for player in game.players.iter().filter(|p| !p.is_minor) {
            let value = game.victory_threat(player.id);
            table += value;
            if is_treated(player.id) {
                mine += value;
            }
        }
        share += if table > 0.0 { mine / table } else { 0.5 };
    }
    share / 2.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 2_100_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    println!(
        "search_dose: {} doses x {maps} mirrored maps, {players}p {width}x{height}, \
         {turns} turns, seed {seed0}",
        DOSES.len()
    );
    println!("  control: {} | statistic: victory-lane progress", CONTROL.0);
    println!("  parity 0.500; above means the dose beats what already shipped\n");

    for (label, review_every, horizon) in DOSES {
        let shares = parallel::map(maps, jobs, move |index| {
            duel(
                (review_every, horizon),
                players,
                width,
                height,
                seed0 + index as u64,
                turns,
            )
        });
        let n = shares.len().max(1) as f64;
        let mean = shares.iter().sum::<f64>() / n;
        let variance = if shares.len() > 1 {
            shares.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let se = (variance / n).sqrt();
        let edge = mean - 0.5;
        let flag = if se > 0.0 && edge.abs() > 2.0 * se {
            "  <-- outside the interval"
        } else {
            ""
        };
        println!("  {label:<22} {mean:.4} +/- {se:.4}   {edge:+.4}{flag}");
    }

    println!(
        "\nA dose outside its interval nominates more compute; it does not promote it.\n\
         Read it against what the repository already measured: the horizon saturates (89% of\n\
         reviews have every branch decided at 120, so they agree by construction), and\n\
         quadrupling reviews was the weakest of the three doublings. A null at 20/120 is the\n\
         PREDICTED result and confirms the instrument. Anything positive earns a\n\
         pre-registered run on WINS, which is what decides it."
    );
}
