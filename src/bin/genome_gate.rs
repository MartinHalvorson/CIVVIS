//! Spend the genome budget at the gate instead of on the ranking.
//!
//! `evolve` evaluates a population of genomes, ranks them on a continuous
//! fitness, and sends **exactly one** — `pop[best]` — to a sequential match
//! against the champion. At `--pop 24 --games 96` that is 2,304 games spent
//! choosing a candidate and about 100 spent testing it: **23 to 1 toward the
//! selection step.**
//!
//! That allocation is only sound if the ranking distinguishes candidates.
//! Measured over four runs at increasing power (`search_probe --selection`,
//! `docs/SUPERHUMAN.md`), it does not: the true spread in win rate between
//! bounded genome perturbations is about **0.013**, and the observed spread
//! never clearly exceeds the error on measuring it.
//!
//! | games/genome | spread | SE | ratio |
//! |---|---|---|---|
//! | 48 | 0.028 | 0.065 | 0.43 |
//! | 480 | 0.025 | 0.020 | 1.25 |
//! | 800 | 0.017 | 0.016 | 1.06 |
//!
//! Ranking reliably on 0.013 needs roughly 5,000 games per candidate. Every
//! budget `evolve` has ever used is two orders of magnitude below that, so
//! `pop[best]` is close to a uniformly random draw from the population — and
//! the other 23 candidates are discarded on that same non-signal.
//!
//! Yet the GA did find a genome worth +49 Elo, through the gate. Both facts fit
//! one model: **it is filtered random search — most candidates are
//! indistinguishable, rare ones are large, and the sequential match is what
//! finds them.** If that is right, the ranking step is not merely wasteful, it
//! is the reason so few candidates are ever tested.
//!
//! So this drops the ranking entirely. Draw a mutation, take it straight to a
//! sequential match against the champion, promote on acceptance, repeat until
//! the budget is gone. The same 2,304 games test **about 23 candidates instead
//! of one.**
//!
//! This is a hypothesis with a cheap falsification: run both at equal total
//! games and compare promotions, then compare the promoted genomes head to
//! head. If ranking carries signal this loses, and that is worth knowing too.
//! The calibration and sequential-gate mathematics are shared with `evolve` so
//! a tool intended to audit allocation cannot accidentally use a looser null.
//!
//! ```text
//! genome_gate --budget 20000 --players 4 --turns 500 --width 24 --height 16
//! ```
use civvis::ai::Weights;
use civvis::evolve::{
    calibrate_promotion_gate, fitness_observations, load_champion, EvoCfg, PromotionGate,
};
use civvis::parallel;

fn flag_present(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// A deterministic stand-in for `evolve::mutate`, which is crate-private.
///
/// Same shape: perturb a subset of genes and clamp to the per-gene bounds
/// evolution respects, so every draw is a play style rather than a broken
/// agent. Seeded from the candidate index alone, so a run is reproducible and
/// two runs at the same budget draw the same candidates.
fn draw(base: &Weights, index: usize, gene_frac: f64, step: f64) -> Weights {
    let mut state = 0x9E3779B97F4A7C15u64 ^ (index as u64).wrapping_mul(0x2545F4914F6CDD1D);
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut genes = base.to_vec();
    for (gene, (low, high)) in genes.iter_mut().zip(Weights::bounds()) {
        if next() < gene_frac {
            // Multiplicative step of +-`step`, so 0.25 reproduces the original
            // +-25% and larger values search further from the incumbent.
            *gene *= (1.0 - step) + 2.0 * step * next();
        }
        *gene = gene.clamp(low, high);
    }
    Weights::from_vec(&genes)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let budget = number(&args, "--budget", 20_000);
    let players = number(&args, "--players", 4).max(2);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed = number(&args, "--seed", 9_000) as u64;
    let jobs = number(&args, "--jobs", 10);
    let chunk = number(&args, "--chunk", 12);
    let max_games = number(&args, "--max-games", 200).max(1);
    let dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "evolved".to_string());

    let mut champion = load_champion(&dir).unwrap_or_default();
    let cfg = EvoCfg {
        generations: 1,
        pop: 1,
        games: chunk,
        players,
        width,
        height,
        max_turns: turns,
        seed,
        threads: jobs,
        dir: dir.clone(),
        speed: civvis::game::default_speed(),
    };
    let calibration_games = number(&args, "--calibrate-games", 240).max(1);
    // The shared production gate fixes a material lift at ten percentage
    // points. Keep the old flag accepting its historic default, but refuse a
    // silent divergence from `evolve`.
    let requested_margin = number(&args, "--margin-pct", 10);
    if requested_margin != 10 {
        eprintln!(
            "genome_gate fixes --margin-pct at 10 so its conservative promotion gate matches evolve"
        );
        std::process::exit(2);
    }
    let mut calibration_epoch = (seed ^ (seed >> 32)) as u32;
    // The shared calibration uses the actual candidate table, including its
    // frozen-default anchor, and turns the one-sided upper bound into H0.
    let mut calibration =
        calibrate_promotion_gate(&champion, &cfg, calibration_epoch, calibration_games);
    println!("H0 calibration: the champion against its own table");
    println!(
        "  {}/{} = {:.3}; conservative H0={:.3}, H1={:.3}",
        calibration.wins(),
        calibration.games(),
        calibration.observed_rate(),
        calibration.gate().null_rate(),
        calibration.gate().alternative_rate(),
    );
    if flag_present(&args, "--calibrate") {
        return;
    }
    // Mutation scale. The default reproduces the first run: +-25% on about a
    // third of genes. 121 draws at that scale produced no candidate worth
    // +72 Elo, which is either an exhausted neighbourhood or too small a step
    // -- one parameter separates them.
    let step = number(&args, "--step-pct", 25) as f64 / 100.0;
    let gene_frac = number(&args, "--gene-pct", 34) as f64 / 100.0;

    println!(
        "genome_gate: budget {budget} games · {players}p {width}x{height} {turns}t · \
         SPRT H0={:.3} (conservative calibration) H1={:.3} bound 2.94, max {max_games} games/candidate",
        calibration.gate().null_rate(),
        calibration.gate().alternative_rate(),
    );
    println!(
        "  mutation: +-{:.0}% on {:.0}% of genes · every game is a gate game, none spent ranking",
        100.0 * step,
        100.0 * gene_frac
    );
    println!();

    // Gate a BATCH of candidates concurrently against the current champion.
    //
    // The first version ran one candidate at a time and did nothing with
    // `--jobs`, because `fitness_observations` plays its games serially: the
    // population was where `evolve` got its parallelism, and dropping the
    // population dropped that too. 20,000 games single-threaded is five hours.
    //
    // Batching restores it without changing the test. Every candidate in a
    // batch faces the same champion, which is exactly what a generation does,
    // and each still walks its own sequential match to its own verdict.
    let (mut spent, mut tested, mut promoted, mut candidate) = (0usize, 0usize, 0usize, 0usize);
    let batch = number(&args, "--batch", jobs);
    'search: while spent < budget {
        let contenders: Vec<(usize, Weights)> = (0..batch)
            .map(|_| {
                candidate += 1;
                (candidate, draw(&champion, candidate, gene_frac, step))
            })
            .collect();
        let champ = champion.clone();
        let gate =
            PromotionGate::from_calibration(calibration.wins(), calibration.games(), max_games);
        let batch_cfg = cfg.clone();
        let results = parallel::map(contenders.len(), jobs, move |i| {
            let (index, contender) = &contenders[i];
            let (mut wins, mut losses) = (0usize, 0usize);
            loop {
                let obs = fitness_observations(
                    contender,
                    std::slice::from_ref(&champ),
                    &batch_cfg,
                    (*index * 97 + wins + losses) as u32,
                    chunk,
                );
                if obs.is_empty() {
                    return (*index, contender.clone(), wins, losses, None);
                }
                wins += obs.iter().filter(|o| o.won).count();
                losses += obs.iter().filter(|o| !o.won).count();
                if let Some(v) = gate.verdict(wins, losses) {
                    return (*index, contender.clone(), wins, losses, Some(v));
                }
            }
        });

        for (index, contender, wins, losses, verdict) in results {
            spent += wins + losses;
            let Some(accepted) = verdict else { continue };
            tested += 1;
            if accepted {
                // Take the first acceptance in the batch and restart against
                // the new champion; later ones in this batch were measured
                // against the old one and are not comparable to it.
                promoted += 1;
                champion = contender;
                calibration_epoch = calibration_epoch.wrapping_add(1);
                calibration =
                    calibrate_promotion_gate(&champion, &cfg, calibration_epoch, calibration_games);
                println!(
                    "  candidate {index}: {wins}-{losses} ACCEPT -> new champion \
                     (after {spent} games, {tested} tested)"
                );
                println!(
                    "    recalibrated {}/{} = {:.3}; conservative H0={:.3}, H1={:.3}",
                    calibration.wins(),
                    calibration.games(),
                    calibration.observed_rate(),
                    calibration.gate().null_rate(),
                    calibration.gate().alternative_rate(),
                );
                println!("    restarting the search against the new champion");
                continue 'search;
            }
        }
        println!("  batch of {batch}: {tested} tested, {spent} games spent, no acceptance");
        if spent >= budget {
            break;
        }
    }

    println!();
    println!("  candidates carried to a verdict   {tested}");
    println!("  promotions                        {promoted}");
    println!("  games spent                       {spent}");
    if tested > 0 {
        println!(
            "  games per candidate               {:.0}",
            spent as f64 / tested as f64
        );
    }
    println!();
    println!(
        "  For comparison, `evolve --pop 24 --games {chunk}` would have spent about
         {} games ranking, to send {} candidate to this same gate.",
        24 * chunk,
        1
    );
    println!(
        "\\nThis is a control, not a promotion path: a genome accepted here has beaten the \
         incumbent on a sequential match and nothing else. Anything it produces still owes \
         a pre-registered ai_eval against the shipped champion before it goes near \
         data/evolved."
    );
}
