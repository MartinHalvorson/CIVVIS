//! Breed the whole genome on the one statistic shown to track winning.
//!
//! Four attempts to make this agent stronger by changing its genome have
//! failed, and the useful part is that they failed for **four different
//! reasons, all of them selection-signal failures rather than search
//! failures**:
//!
//! | attempt | outcome | why |
//! |---|---|---|
//! | ~1000 rounds of live `evolve` | no gain | ratings carried negative information |
//! | breed the policy appetites | +0.0138 ± 0.0138 | fitness was 80% score share |
//! | transfer the best-rated league genome | **−98 Elo, p=0.0034** | rating pool selected something systematically bad |
//! | raise `settler_min_pop` to 5 | 12–15 on wins | +0.019 of score share that did not convert |
//!
//! None of those searched badly. Each optimised a quantity that is not
//! winning. `docs/GENOME.md` establishes why score cannot be that quantity: it
//! classifies the winner at AUC 0.949 **observationally**, yet an intervention
//! moves it +0.011 to +0.017 while wins sit at parity — under every functional
//! from the mean through top-of-table, so convexity is not the fix either. It
//! is a correlate, and a search over a correlate optimises whichever correlate
//! is cheapest to move. That is `docs/SUPERHUMAN.md` §0 one level up.
//!
//! `Game::victory_threat` — progress along the empire's best enabled victory
//! lane — is the only statistic that passed both halves of the check:
//! **+0.0031 where wins say parity, +0.0280 where wins say a significant
//! positive.** Both halves were needed; an inert statistic passes the null test
//! for free.
//!
//! And the genome does have purchase: a whole-genome swap moved wins at
//! p=0.0034. Downward, but that is causal purchase the block-wise ablation
//! missed. So the search is worth running once, with a signal that points at
//! the right thing.
//!
//! ```text
//! genome_breed --pop 10 --gens 6 --maps 12 --seed 2400000
//! ```
//!
//! ## What this will and will not establish
//!
//! Fitness is lane progress against the **shipped default genome** as a fixed
//! opponent of known strength, paired and seat-mirrored. Every generation draws
//! a fresh map set, the champion is re-measured on a disjoint holdout beside
//! the shipped genome, and the **selection-minus-holdout gap is printed** —
//! that number is how much of the champion is fitting rather than strength.
//!
//! **A champion here is a nomination, not a result.** Lane progress is
//! validated on two test cases, which is two. The verdict is a pre-registered
//! `ai_eval` run on **wins**, and everything in this repository that looked
//! like a finding and was not died at exactly that step.
//!
//! One scope limit worth stating: this breeds at `AdvancedAi` cost, while the
//! −98 Elo genome result was measured at the `strategic_deep` budget. Genome
//! effects need not be budget-invariant, so a champion found here has to be
//! confirmed at the budget it would ship into.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::rng::Rng;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One mirrored map pair. Returns the candidate's share of the table's
/// victory-lane progress, averaged over both seat parities.
///
/// No win term is folded in. Mixing a binary indicator back into a continuous
/// statistic reintroduces exactly the variance the continuous one exists to
/// avoid, which is the flaw that made an earlier breeder a random walk.
fn duel(
    candidate: &Weights,
    players: usize,
    w: i32,
    h: i32,
    seed: u64,
    turns: u32,
    on_wins: bool,
) -> f64 {
    let mut share = 0.0;
    for treated in 0..2usize {
        let mut game = Game::new(players, w, h, seed, turns, 0);
        let stock = Weights::default();
        let mut mine: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, candidate);
        let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
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
                    mine[pid].take_turn(&mut game, pid);
                } else {
                    rivals[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if on_wins {
            // UNBIASED and noisy. A capped game with no victor is 0.5: it says
            // nothing about either arm, and scoring it as a loss would be a
            // bias of exactly the kind this mode exists to avoid.
            share += match game.winner.map(is_treated) {
                Some(true) => 1.0,
                Some(false) => 0.0,
                None => 0.5,
            };
        } else {
            let mut ours = 0.0;
            let mut table = 0.0;
            for player in game.players.iter().filter(|p| !p.is_minor) {
                let lane = game.victory_threat(player.id);
                table += lane;
                if is_treated(player.id) {
                    ours += lane;
                }
            }
            share += if table > 0.0 { ours / table } else { 0.5 };
        }
    }
    share / 2.0
}

/// Mean lane-progress share and the standard error of that mean.
fn fitness(
    candidate: &Weights,
    players: usize,
    w: i32,
    h: i32,
    maps: usize,
    seed0: u64,
    turns: u32,
    jobs: usize,
    on_wins: bool,
) -> (f64, f64) {
    let genome = candidate.clone();
    let shares = parallel::map(maps, jobs, move |index| {
        duel(&genome, players, w, h, seed0 + index as u64, turns, on_wins)
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let pop_size = number(&args, "--pop", 10).max(2);
    let gens = number(&args, "--gens", 6);
    let maps = number(&args, "--maps", 12);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 2_400_000) as u64;
    let holdout_maps = number(&args, "--holdout-maps", 48);
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    // Breeding on WINS is unbiased and noisy; breeding on lane progress is
    // cheap and BIASED. That distinction is the whole finding: noise
    // misdirects selection randomly and averages out over generations, while
    // bias misdirects it consistently and converges harder on the wrong thing.
    // A lane-bred champion measured 8 map directions to 30 against the shipped
    // genome at p=0.0005.
    let on_wins = args.iter().any(|arg| arg == "--fitness-wins");

    let bounds = Weights::bounds();
    let shipped = Weights::default().to_vec();

    // --wins <40 comma-separated genes>: decide a champion on map directions
    // and a sign test over WINS, which is what lane progress only nominates.
    if let Some(spec) = args
        .iter()
        .position(|arg| arg == "--wins")
        .and_then(|index| args.get(index + 1))
    {
        let genes: Vec<f64> = spec
            .split(',')
            .filter_map(|piece| piece.trim().parse().ok())
            .collect();
        if genes.len() != shipped.len() {
            eprintln!(
                "genome_breed: --wins needs {} comma-separated genes, got {}",
                shipped.len(),
                genes.len()
            );
            std::process::exit(2);
        }
        let champion = Weights::from_vec(&genes);
        println!(
            "wins verdict: champion vs shipped, {maps} maps x 2 directions, {players}p,              {turns} turns, seed {seed0}"
        );
        let results = parallel::map(maps, jobs, move |index| {
            let seed = seed0 + index as u64;
            let mut out = [None, None];
            for (slot, treated) in (0..2usize).enumerate() {
                let mut game = Game::new(players, width, height, seed, turns, 0);
                let stock = Weights::default();
                let mut mine: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &champion);
                let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
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
                            mine[pid].take_turn(&mut game, pid);
                        } else {
                            rivals[pid].take_turn(&mut game, pid);
                        }
                        if game.winner.is_none() && game.current == pid {
                            let _ = game.apply(pid, &Action::EndTurn);
                        }
                    }
                }
                // A capped game with no victor says nothing about either arm.
                out[slot] = game.winner.map(is_treated);
            }
            out
        });
        let (mut up, mut down, mut neutral, mut won, mut decisive) = (0u32, 0u32, 0u32, 0u32, 0u32);
        for pair in &results {
            for arm in pair.iter() {
                match arm {
                    Some(true) => {
                        decisive += 1;
                        won += 1;
                    }
                    Some(false) => decisive += 1,
                    None => {}
                }
            }
            match (pair[0], pair[1]) {
                (Some(true), Some(true)) => up += 1,
                (Some(false), Some(false)) => down += 1,
                _ => neutral += 1,
            }
        }
        // Exact two-sided sign test over the maps that broke.
        let n = up + down;
        let p = if n == 0 {
            1.0
        } else {
            let extreme = up.min(down);
            let mut log_c = 0.0f64;
            let mut tail = 0.0f64;
            for k in 0..=extreme {
                if k > 0 {
                    log_c += ((n - k + 1) as f64).ln() - (k as f64).ln();
                }
                tail += (log_c - (n as f64) * std::f64::consts::LN_2).exp();
            }
            (2.0 * tail).min(1.0)
        };
        println!(
            "  decisive games   {won}/{decisive} ({:.1}%)",
            100.0 * won as f64 / decisive.max(1) as f64
        );
        println!("  map directions   {up} for / {down} against / {neutral} neutral");
        println!("  sign test        p = {p:.4}");
        println!(
            "
  {}",
            if up > down && p < 0.05 {
                "PASS under the pre-registered rule. Confirm at the strategic_deep budget next."
            } else {
                "NULL under the pre-registered rule. The shipped genome stands, and a lane-progress                  edge that does not convert is exactly what score share did."
            }
        );
        return;
    }
    let names = Weights::gene_names();
    let mut rng = Rng::new(seed0 ^ 0x5DEE_CE66_D1EB_51DA);

    println!(
        "genome_breed: {pop_size} genomes x {gens} generations, fitness on {maps} mirrored maps \
         ({players}p {width}x{height}, {turns} turns) against the shipped genome, seed {seed0}"
    );
    println!(
        "  statistic: {}",
        if on_wins { "WIN RATE -- unbiased, noisy; parity 0.500" } else { "victory-lane progress -- cheap, BIASED; parity 0.500" }
    );
    println!("  the shipped genome is member 0 of generation 0\n");

    // Seed the shipped genome so the search must beat what it replaces, then
    // perturb rather than reroll: a uniform draw over 40 genes lands nowhere
    // near a playable agent, and the league transfer showed a genome-scale jump
    // can be worth -98 Elo.
    let mut pop: Vec<Vec<f64>> = vec![shipped.clone()];
    while pop.len() < pop_size {
        let mut genes = shipped.clone();
        for (gene, (lo, hi)) in genes.iter_mut().zip(bounds) {
            if rng.chance(0.25) {
                *gene = (*gene + rng.uniform(-0.25, 0.25) * (hi - lo)).clamp(lo, hi);
            }
        }
        pop.push(genes);
    }

    // --climb: a (1+1) paired hill climb, which spends games far better than a
    // population does at this cost.
    //
    // The population version has two leaks. It scores every genome against the
    // SHIPPED agent and then compares those estimates to each other, so any two
    // candidates are compared through two independent noisy numbers rather than
    // head to head. And it takes the maximum of pop*gens draws, which is worth
    // about +2 SE by construction — the last run's selection score was 0.5875
    // against a 0.5167 holdout, a +0.0708 gap that was essentially all of that.
    //
    // A hill climb removes both. Each step plays ONE mutant directly against
    // the incumbent on the same maps, mirrored, and accepts only on a
    // significant margin. There is one comparison per step, so there is no
    // maximum-of-many inflation, and the comparison is paired at the map level.
    if args.iter().any(|arg| arg == "--climb") {
        let steps = number(&args, "--steps", 12);
        // The climb scores WINS unconditionally -- see the match on
        // `game.winner` below -- so say so here. The header printed above
        // reads the `--fitness-wins` flag, which a climb run does not need to
        // pass, and it therefore mislabelled the first climb output as lane
        // progress. The data was right and the label was wrong, which is the
        // second time in this work a tool has misdescribed its own output.
        println!("  NOTE: --climb always scores WINS; ignore any statistic line above.");
        let mut incumbent = shipped.clone();
        let mut accepted = 0usize;
        println!("  (1+1) paired hill climb: {steps} steps, mutant vs incumbent head to head\n");
        for step in 0..steps {
            let mut mutant = incumbent.clone();
            for (index, (lo, hi)) in bounds.iter().enumerate() {
                if rng.chance(0.20) {
                    mutant[index] =
                        (mutant[index] + rng.uniform(-0.20, 0.20) * (hi - lo)).clamp(*lo, *hi);
                }
            }
            let step_seed = seed0 + 10_000 * (step as u64 + 1);
            let (mutant_w, incumbent_w) =
                (Weights::from_vec(&mutant), Weights::from_vec(&incumbent));
            // Head to head on the same maps: the incumbent is the opponent, not
            // a separately-estimated reference.
            let shares = parallel::map(maps, jobs, move |index| {
                let seed = step_seed + index as u64;
                let mut got = 0.0;
                for treated in 0..2usize {
                    let mut game = Game::new(players, width, height, seed, turns, 0);
                    let mut a: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &mutant_w);
                    let mut b: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &incumbent_w);
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
                                a[pid].take_turn(&mut game, pid);
                            } else {
                                b[pid].take_turn(&mut game, pid);
                            }
                            if game.winner.is_none() && game.current == pid {
                                let _ = game.apply(pid, &Action::EndTurn);
                            }
                        }
                    }
                    got += match game.winner.map(is_treated) {
                        Some(true) => 1.0,
                        Some(false) => 0.0,
                        None => 0.5,
                    };
                }
                got / 2.0
            });
            let n = shares.len().max(1) as f64;
            let mean = shares.iter().sum::<f64>() / n;
            let var = if shares.len() > 1 {
                shares.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
            } else {
                0.0
            };
            let se = (var / n).sqrt();
            // Accept only on a margin. Accepting on mean > 0.5 alone would let
            // the climb ratchet upward on noise, which is the same
            // maximum-of-many failure in slower motion.
            let take = se > 0.0 && (mean - 0.5) > 2.0 * se;
            println!(
                "  step {step:2}  mutant {mean:.4} +/- {se:.4}  ({:+.1} SE)  {}",
                if se > 0.0 { (mean - 0.5) / se } else { 0.0 },
                if take { "ACCEPT" } else { "reject" }
            );
            if take {
                incumbent = mutant;
                accepted += 1;
            }
        }
        println!("\n  {accepted} of {steps} steps accepted");
        if accepted == 0 {
            println!(
                "  The incumbent survived every challenge at a 2 SE bar. On {maps} maps a step\n                   resolves about {:.3}, so effects smaller than roughly {:.2} are invisible here.",
                (0.25f64 / (maps as f64 * 2.0)).sqrt(),
                2.0 * (0.25f64 / (maps as f64 * 2.0)).sqrt()
            );
        } else {
            println!("  genome: {}", incumbent.iter().map(|g| format!("{g:.3}")).collect::<Vec<_>>().join(","));
            println!("  Re-measure this against the SHIPPED genome on fresh maps before believing it:");
            println!("  a chain of accepted steps is still a chain of comparisons against a moving target.");
        }
        return;
    }

    let mut best = (0.0f64, shipped.clone());
    for generation in 0..gens {
        let map_seed = seed0 + 1_000 * (generation as u64 + 1);
        let mut scored: Vec<(f64, f64, Vec<f64>)> = pop
            .iter()
            .map(|genes| {
                let (fit, se) = fitness(
                    &Weights::from_vec(genes),
                    players,
                    width,
                    height,
                    maps,
                    map_seed,
                    turns,
                    jobs,
                    on_wins,
                );
                (fit, se, genes.clone())
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        if scored[0].0 > best.0 {
            best = (scored[0].0, scored[0].2.clone());
        }
        let mean: f64 = scored.iter().map(|s| s.0).sum::<f64>() / scored.len() as f64;
        println!(
            "  gen {generation:2}  best {:.4} +/- {:.4}  mean {mean:.4}",
            scored[0].0, scored[0].1
        );

        let keep = (pop_size / 2).max(1);
        let mut next: Vec<Vec<f64>> = vec![scored[0].2.clone()];
        while next.len() < pop_size {
            let a = &scored[rng.below(keep)].2;
            let b = &scored[rng.below(keep)].2;
            let mut child = a.clone();
            for (index, (lo, hi)) in bounds.iter().enumerate() {
                if rng.chance(0.5) {
                    child[index] = b[index];
                }
                if rng.chance(0.08) {
                    child[index] = rng.uniform(*lo, *hi);
                } else if rng.chance(0.30) {
                    child[index] += rng.uniform(-0.15, 0.15) * (hi - lo);
                }
                child[index] = child[index].clamp(*lo, *hi);
            }
            next.push(child);
        }
        pop = next;
    }

    // Disjoint maps the search never saw, against the shipped genome.
    let holdout_seed = seed0 + 700_000;
    let champion = Weights::from_vec(&best.1);
    let (holdout, holdout_se) = fitness(
        &champion, players, width, height, holdout_maps, holdout_seed, turns, jobs, on_wins,
    );

    println!("\nchampion, genes that moved from the shipped value:");
    let mut moved = 0;
    for (index, name) in names.iter().enumerate() {
        if (best.1[index] - shipped[index]).abs() > 1e-9 {
            println!(
                "  {name:<20} {:>8.3} -> {:>8.3}",
                shipped[index], best.1[index]
            );
            moved += 1;
        }
    }
    if moved == 0 {
        println!("  none -- the shipped genome won its own search");
    }

    let edge = holdout - 0.5;
    println!(
        "\n  selection score  {:.4} (on its own maps)\n  \
         holdout score    {holdout:.4} +/- {holdout_se:.4} ({holdout_maps} disjoint maps, seed {holdout_seed})\n  \
         fitted gap       {:+.4}\n  \
         holdout edge     {edge:+.4}  ({:.1} SE)",
        best.0,
        best.0 - holdout,
        if holdout_se > 0.0 { edge / holdout_se } else { 0.0 }
    );

    if holdout_se > 0.0 && edge.abs() < 2.0 * holdout_se {
        println!(
            "\n  => INSIDE the interval on maps it did not select against. This champion is not\n     \
             distinguishable from the shipped genome; do NOT queue an ai_eval on it. A large\n     \
             fitted gap with a null holdout is the signature of fitting the selection maps."
        );
    } else if edge > 0.0 {
        println!(
            "\n  => outside the interval and positive. THIS earns a pre-registered ai_eval on\n     \
             WINS at 100+ mirrored maps, which decides it -- and a confirmation at the\n     \
             strategic_deep budget, since genome effects need not be budget-invariant."
        );
    } else {
        println!("\n  => outside the interval and NEGATIVE. The search made the agent worse.");
    }
}
