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
//! searches 40 genes against a *churning* population, and about a thousand
//! rounds of it produced no measurable gain (`docs/RATING.md`). Here the
//! search space is 8 genes and the opponent is **fixed** at a strength that
//! has been measured, so a fitness difference means what it says.
//!
//! ```text
//! policy_breed --pop 12 --gens 10 --maps 12 --seed 500000
//! ```
//!
//! ## Selection must out-resolve its own noise — the first build did not
//!
//! The first version scored genomes on **win rate** over 12 maps and was
//! retired after three generations of 0.500, 0.542, 0.500. That is not a
//! search converging; it is a random walk, and the arithmetic says so. A win
//! rate over 24 games has a standard error of **0.102**, while the largest
//! real effects this repository has ever measured are +0.053 (warm branches)
//! and +0.065 (`strategic_deep`). Selecting on a quantity whose noise is twice
//! the signal cannot climb, and resolving a 0.05 effect to three standard
//! errors would need about **865 games per genome**.
//!
//! **This is the documented failure of the repository's own breeder** — a
//! thousand rounds of live evolution produced no measurable gain because
//! selection had no signal (`docs/RATING.md`). It is very easy to rebuild by
//! accident.
//!
//! So fitness is `0.8 * terminal score share + 0.2 * win rate`. Score is
//! continuous and far tighter, so it ranks genomes within a budget a search
//! can actually afford; the win term keeps a victory worth more than the score
//! it ends on. It is a **proxy**, and `docs/EVAL.md` is explicit that wins and
//! score measure different things — so score *selects* and only `policy_eval`
//! *decides*.
//!
//! The champion is re-measured on a **disjoint** holdout beside the shipped
//! appetites, and the selection-minus-holdout gap is printed: that number is
//! how much of the champion is fitting. **Promote only on a fresh
//! pre-registered `policy_eval` run over 100+ mirrored maps.**
//!
//! The paired-play helper is duplicated from `policy_eval.rs` rather than
//! shared: hoisting it would mean editing `src/lib.rs`, which another open PR
//! claims, and forty lines of duplication in two eval binaries is the cheaper
//! of the two costs.
use civvis::ai::{AdvancedAi, Ai, PolicyDeck, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::rng::Rng;

// The appetites are fields on `Weights` but deliberately NOT part of the GA
// vector -- scrambling the whole block costs +0.0006 +/- 0.0229 on the
// wins-tracking statistic, so they are set directly rather than by index.
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

/// One mirrored map pair. Returns the candidate's share of the table's
/// terminal score, averaged over the two directions.
///
/// **Selection reads score, not wins, and that is a deliberate split.** A win
/// rate over a handful of games is almost pure noise: at 12 maps its standard
/// error is 0.102, while the real effects this repository has measured are
/// +0.053 (warm branches) and +0.065 (`strategic_deep`). Selecting on a
/// quantity whose noise is twice the signal is a random walk, and it is the
/// documented reason a thousand rounds of live evolution produced no
/// measurable gain (`docs/RATING.md`) -- breeding on noise. Resolving a 0.05
/// win-rate effect to three standard errors needs about 865 games *per
/// genome*, which no search can afford.
///
/// Terminal score share is continuous and far tighter, so it ranks genomes
/// with the budget a search actually has. It is a *proxy*: `docs/EVAL.md`
/// records that wins and score measure different things, and a change that
/// re-routes victory lanes moves wins while score stays flat. So score selects
/// and only `policy_eval` decides -- the champion is still judged on wins over
/// 100+ mirrored maps before anything is promoted.
fn duel(candidate: &Weights, players: usize, w: i32, h: i32, seed: u64, turns: u32) -> f64 {
    let mut share = 0.0;
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
        let mut mine = 0.0;
        let mut table = 0.0;
        for player in game.players.iter().filter(|p| !p.is_minor) {
            let score = game.score(player.id) as f64;
            table += score;
            if is_treated(player.id) {
                mine += score;
            }
        }
        // A win is worth more than the score it ends on, so it still counts --
        // just not as the whole signal.
        let won = if game.winner.is_some_and(is_treated) { 1.0 } else { 0.0 };
        share += 0.8 * (mine / table.max(1.0)) + 0.2 * won;
    }
    share / 2.0
}

/// The candidate's mean score share against the legacy arm, **with the standard
/// error of that mean**.
///
/// The SE is not decoration. The first build of this search selected on a
/// statistic whose noise was twice any real effect, and nothing in its output
/// said so -- three generations of 0.500, 0.542, 0.500 looked like a search
/// until the arithmetic was done by hand afterwards. A fitness that reports its
/// own uncertainty cannot hide that failure a second time.
fn fitness(
    candidate: &Weights,
    players: usize,
    w: i32,
    h: i32,
    maps: usize,
    seed0: u64,
    turns: u32,
    jobs: usize,
) -> (f64, f64) {
    let genome = candidate.clone();
    let shares = parallel::map(maps, jobs, move |index| {
        duel(&genome, players, w, h, seed0 + index as u64, turns)
    });
    let n = shares.len().max(1) as f64;
    let mean = shares.iter().sum::<f64>() / n;
    let variance = if shares.len() > 1 {
        shares.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    (mean, (variance / n).sqrt())
}

fn with_policy_genes(genes: &[f64; 8]) -> Weights {
    Weights {
        pol_food: genes[0],
        pol_production: genes[1],
        pol_gold: genes[2],
        pol_science: genes[3],
        pol_culture: genes[4],
        pol_faith: genes[5],
        pol_military: genes[6],
        pol_swap_margin: genes[7],
        policy_deck: civvis::ai::PolicyDeck::Live,
        ..Weights::default()
    }
}

/// (lo, hi) for each appetite, in the order `with_policy_genes` reads them.
const POLICY_BOUNDS: [(f64, f64); 8] = [
    (0.0, 3.0),
    (0.0, 3.0),
    (0.0, 3.0),
    (0.0, 3.0),
    (0.0, 3.0),
    (0.0, 3.0),
    (0.0, 0.5),
    (0.0, 1.0),
];

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

    // Evaluate one named genome against the shipped appetites and stop. The
    // search itself is expensive and often unnecessary: once a candidate
    // exists, the only question left is whether it survives maps it did not
    // select against, and that is two fitness calls rather than eight
    // generations.
    if let Some(spec) = args
        .iter()
        .position(|arg| arg == "--genes")
        .and_then(|index| args.get(index + 1))
    {
        let parsed: Vec<f64> = spec
            .split(',')
            .filter_map(|piece| piece.trim().parse().ok())
            .collect();
        if parsed.len() != 8 {
            eprintln!("policy_breed: --genes needs 8 comma-separated numbers, got {}", parsed.len());
            std::process::exit(2);
        }
        let mut genes = [0.0f64; 8];
        genes.copy_from_slice(&parsed);
        let holdout_seed = seed0 + 900_000;
        let (mine, mine_se) = fitness(
            &with_policy_genes(&genes), players, width, height, holdout_maps, holdout_seed, turns, jobs,
        );
        let (shipped, shipped_se) = fitness(
            &Weights::default(), players, width, height, holdout_maps, holdout_seed, turns, jobs,
        );
        let edge = mine - shipped;
        let edge_se = (mine_se.powi(2) + shipped_se.powi(2)).sqrt();
        println!("candidate  {mine:.4} +/- {mine_se:.4}");
        println!("shipped    {shipped:.4} +/- {shipped_se:.4}");
        println!(
            "edge       {edge:+.4} +/- {edge_se:.4}   ({:.1} SE) over {holdout_maps} disjoint maps, seed {holdout_seed}",
            if edge_se > 0.0 { edge / edge_se } else { 0.0 }
        );
        if edge_se > 0.0 && edge.abs() < 2.0 * edge_se {
            println!("=> inside the interval: not distinguishable from the shipped appetites.");
        }
        return;
    }

    let bounds: Vec<(f64, f64)> = POLICY_BOUNDS.to_vec();
    let mut rng = Rng::new(seed0 ^ 0x9E37_79B9_7F4A_7C15);

    // Seat the shipped hand-set appetites so the search must beat what it
    // replaces, then fill the rest of the population uniformly at random.
    let stock = Weights::default();
    let mut pop: Vec<[f64; 8]> = Vec::with_capacity(pop_size);
    let seed_genes = [
        stock.pol_food,
        stock.pol_production,
        stock.pol_gold,
        stock.pol_science,
        stock.pol_culture,
        stock.pol_faith,
        stock.pol_military,
        stock.pol_swap_margin,
    ];
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
    println!("  fitness is 0.8*score share + 0.2*win rate; parity is 0.500");
    println!("  the shipped appetites are genome 0 of generation 0");

    let mut best = (0.0f64, seed_genes);
    for generation in 0..gens {
        // Every genome of a generation meets the same maps, and a fresh set
        // each generation so a champion cannot survive on one lucky board.
        let map_seed = seed0 + 1_000 * (generation as u64 + 1);
        let mut scored: Vec<(f64, [f64; 8])> = pop
            .iter()
            .map(|genes| {
                let candidate = with_policy_genes(genes);
                let (fit, _se) = fitness(
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
    let (holdout, holdout_se) = fitness(
        &champion, players, width, height, holdout_maps, holdout_seed, turns, jobs,
    );
    let (shipped_holdout, shipped_se) = fitness(
        &Weights::default(), players, width, height, holdout_maps, holdout_seed, turns, jobs,
    );

    println!("\nchampion");
    for (k, name) in GENE_NAMES.iter().enumerate() {
        println!("  pol_{name:<12} {:.4}", best.1[k]);
    }
    println!("  selection score  {:.3} (on its own maps)", best.0);
    println!("  holdout score    {holdout:.3} +/- {holdout_se:.3} ({holdout_maps} disjoint maps, seed {holdout_seed})");
    println!("  shipped appetites{shipped_holdout:>7.3} +/- {shipped_se:.3} (same holdout maps)");
    // The only comparison that matters, and the only one with an error bar.
    let edge = holdout - shipped_holdout;
    let edge_se = (holdout_se.powi(2) + shipped_se.powi(2)).sqrt();
    println!(
        "  champion - shipped {edge:+.3} +/- {edge_se:.3}  ({:.1} SE)",
        if edge_se > 0.0 { edge / edge_se } else { 0.0 }
    );
    if edge_se > 0.0 && edge.abs() < 2.0 * edge_se {
        println!(
            "  => INSIDE the interval. This champion is not distinguishable from the shipped\n     \
             appetites on maps it did not select against. Do NOT queue a policy_eval on it."
        );
    } else if edge > 0.0 {
        println!(
            "  => outside the interval and positive. THIS earns a pre-registered policy_eval\n     \
             at 100+ mirrored maps on a fresh seed -- which decides it, not this number."
        );
    }
    println!(
        "\nfitted-to-the-maps gap: {:.3}. A champion that keeps its edge on the holdout has \
         earned a pre-registered policy_eval run; one that does not has been fitted.",
        best.0 - holdout
    );
}
