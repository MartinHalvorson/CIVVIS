//! Breed the eight policy-deck genes against a fixed opponent of known strength.
//!
//! Two measurements set this up, both at 120 mirrored maps and 500 turns:
//!
//! | arms | result |
//! |---|---|
//! | legacy 20-card deck vs no cards at all | 23 map directions to 6, p=0.0023 |
//! | the live valuation vs legacy | 18 to 15, p=0.7283 |
//!
//! So the card layer is worth a great deal and the live valuation captured
//! none of the headroom above the old list. That is **not** evidence the gene
//! axis is worthless: `pol_food` through `pol_swap_margin` were set by hand,
//! and one hand-picked point in an eight-dimensional space losing to nothing
//! says nothing about the best point in it. Nobody has ever bred them.
//!
//! Why this is better conditioned than `civvis evolve`. The whole-genome GA
//! searches 48 genes against a *churning* population, and about a thousand
//! rounds of it produced no measurable gain (`docs/RATING.md`). Here the
//! search space is 8 genes and the opponent is **fixed** at a strength that
//! has been measured, so a fitness difference means what it says.
//!
//! ```text
//! policy_breed --pop 12 --gens 10 --maps 12 --seed 500000
//! ```
//!
//! **Selection is cheap and the verdict is not.** Fitness runs on few maps on
//! purpose — selection tolerates noise, since a lucky genome has to keep
//! winning to survive. The champion is then re-measured on a **disjoint**
//! holdout seed set, and both numbers are printed. Read the gap: a champion
//! far better on its selection maps than on the holdout was fitted to the
//! maps, and `docs/EVAL.md` records that nothing under ~100 maps decides
//! anything here. **Promote only on a fresh pre-registered `policy_eval` run.**
//!
//! The paired-play helper is duplicated from `policy_eval.rs` rather than
//! shared: hoisting it would mean editing `src/lib.rs`, which another open PR
//! claims, and forty lines of duplication in two eval binaries is the cheaper
//! of the two costs.
use civvis::ai::{AdvancedAi, Ai, PolicyDeck, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::rng::Rng;

/// Indices into `Weights::to_vec` that this search is allowed to move.
const POLICY_GENES: [usize; 8] = [40, 41, 42, 43, 44, 45, 46, 47];
const GENE_NAMES: [&str; 8] = [
    "food",
    "production",
    "gold",
    "science",
    "culture",
    "faith",
    "military",
    "swap_margin",
];

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One mirrored map pair. Returns games won by the candidate, out of two.
fn duel(candidate: &Weights, players: usize, w: i32, h: i32, seed: u64, turns: u32) -> u32 {
    let mut won = 0;
    for treated in 0..2usize {
        let mut game = Game::new(players, w, h, seed, turns, 0);
        let control = Weights {
            policy_deck: PolicyDeck::Legacy,
            ..Weights::default()
        };
        let mut treatment: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, candidate);
        let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &control);
        let is_treated = |pid: usize| pid % 2 == treated;
        for _ in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if is_treated(pid) {
                    treatment[pid].take_turn(&mut game, pid);
                } else {
                    rivals[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some_and(is_treated) {
            won += 1;
        }
    }
    won
}

/// Share of decisive games the candidate takes from the legacy arm.
fn fitness(
    candidate: &Weights,
    players: usize,
    w: i32,
    h: i32,
    maps: usize,
    seed0: u64,
    turns: u32,
    jobs: usize,
) -> f64 {
    let genome = candidate.clone();
    let wins = parallel::map(maps, jobs, move |index| {
        duel(&genome, players, w, h, seed0 + index as u64, turns)
    });
    let total: u32 = wins.iter().sum();
    total as f64 / (2 * maps) as f64
}

fn with_policy_genes(genes: &[f64; 8]) -> Weights {
    let mut v = Weights::default().to_vec();
    for (slot, gene) in POLICY_GENES.iter().zip(genes) {
        v[*slot] = *gene;
    }
    Weights::from_vec(&v)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let pop_size = number(&args, "--pop", 12);
    let gens = number(&args, "--gens", 10);
    let maps = number(&args, "--maps", 12);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 500_000) as u64;
    let holdout_maps = number(&args, "--holdout-maps", 60);
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    let all_bounds = Weights::bounds();
    let bounds: Vec<(f64, f64)> = POLICY_GENES.iter().map(|i| all_bounds[*i]).collect();
    let mut rng = Rng::new(seed0 ^ 0x9E37_79B9_7F4A_7C15);

    // Seat the shipped hand-set appetites so the search must beat what it
    // replaces, then fill the rest of the population uniformly at random.
    let shipped = Weights::default().to_vec();
    let mut pop: Vec<[f64; 8]> = Vec::with_capacity(pop_size);
    let mut seed_genes = [0.0f64; 8];
    for (k, index) in POLICY_GENES.iter().enumerate() {
        seed_genes[k] = shipped[*index];
    }
    pop.push(seed_genes);
    while pop.len() < pop_size {
        let mut genes = [0.0f64; 8];
        for (k, (lo, hi)) in bounds.iter().enumerate() {
            genes[k] = rng.uniform(*lo, *hi);
        }
        pop.push(genes);
    }

    println!(
        "policy_breed: {pop_size} genomes x {gens} generations, fitness on {maps} mirrored maps \
         ({players}p {width}x{height}, {turns} turns) against the legacy deck, seed {seed0}"
    );
    println!("  parity is 50.0%; the shipped appetites are genome 0 of generation 0");

    let mut best = (0.0f64, seed_genes);
    for generation in 0..gens {
        // Every genome of a generation meets the same maps, and a fresh set
        // each generation so a champion cannot survive on one lucky board.
        let map_seed = seed0 + 1_000 * (generation as u64 + 1);
        let mut scored: Vec<(f64, [f64; 8])> = pop
            .iter()
            .map(|genes| {
                let candidate = with_policy_genes(genes);
                let fit = fitness(
                    &candidate, players, width, height, maps, map_seed, turns, jobs,
                );
                (fit, *genes)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        if scored[0].0 > best.0 {
            best = scored[0];
        }
        let mean: f64 = scored.iter().map(|s| s.0).sum::<f64>() / scored.len() as f64;
        println!(
            "  gen {generation:2}  best {:.3}  mean {:.3}  best genes [{}]",
            scored[0].0,
            mean,
            scored[0]
                .1
                .iter()
                .map(|g| format!("{g:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        // Elitist: the top half breeds, the best genome survives untouched.
        let keep = (pop_size / 2).max(1);
        let mut next: Vec<[f64; 8]> = vec![scored[0].1];
        while next.len() < pop_size {
            let a = scored[rng.below(keep)].1;
            let b = scored[rng.below(keep)].1;
            let mut child = [0.0f64; 8];
            for k in 0..8 {
                let (lo, hi) = bounds[k];
                child[k] = if rng.chance(0.5) { a[k] } else { b[k] };
                if rng.chance(0.10) {
                    child[k] = rng.uniform(lo, hi);
                } else if rng.chance(0.35) {
                    child[k] += rng.uniform(-0.12, 0.12) * (hi - lo);
                }
                child[k] = child[k].clamp(lo, hi);
            }
            next.push(child);
        }
        pop = next;
    }

    // Disjoint maps the search never saw. The gap between this and the
    // selection score is how much of the champion is real.
    let champion = with_policy_genes(&best.1);
    let holdout_seed = seed0 + 900_000;
    let holdout = fitness(
        &champion,
        players,
        width,
        height,
        holdout_maps,
        holdout_seed,
        turns,
        jobs,
    );
    let shipped_holdout = fitness(
        &Weights::default(),
        players,
        width,
        height,
        holdout_maps,
        holdout_seed,
        turns,
        jobs,
    );

    println!("\nchampion");
    for (k, name) in GENE_NAMES.iter().enumerate() {
        println!("  pol_{name:<12} {:.4}", best.1[k]);
    }
    println!("  selection score  {:.3} (on its own maps)", best.0);
    println!("  holdout score    {holdout:.3} ({holdout_maps} disjoint maps, seed {holdout_seed})");
    println!("  shipped appetites{shipped_holdout:>7.3} (same holdout maps)");
    println!(
        "\nfitted-to-the-maps gap: {:.3}. A champion that keeps its edge on the holdout has \
         earned a pre-registered policy_eval run; one that does not has been fitted.",
        best.0 - holdout
    );
}
