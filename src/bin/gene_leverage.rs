//! What does each block of the genome cost when you get it wrong?
//!
//! `gene_probe` answered which genes can change a game and eleven of forty-eight
//! cannot. Then the opening book — the most reachable block by that measure,
//! `open0` diverging 12/12 by turn 8 — turned out to be worth **nothing**:
//! deleting it entirely costs −0.0028 ± 0.0164. So divergence is necessary for
//! a gene to matter and nowhere near sufficient, and ranking work by it is a
//! mistake I made and paid for.
//!
//! This ranks by the right thing. For each block of related genes, replace the
//! shipped values with **random draws from their own bounds** and measure what
//! that costs against the shipped agent, paired and seat-mirrored. Averaged
//! over several draws, the loss is a Sobol-flavoured importance: *how much does
//! getting this block right matter at all?*
//!
//! It reads in the useful direction. A block whose randomisation costs nothing
//! is **settled** — the shipped values are not carrying anything, so no search
//! over them can pay, whatever a divergence probe says. A block whose
//! randomisation costs a lot is one where the shipped values are load-bearing,
//! which is the only situation in which better values can exist.
//!
//! ```text
//! gene_leverage --maps 16 --draws 3
//! ```
//!
//! **A null here is the informative outcome.** Every parameter-tuning attempt
//! on this agent has returned null — policy appetites three ways, the opening
//! book two ways, the war threshold, a thousand rounds of whole-genome
//! evolution — while every promoted gain came from more counterfactual rollout.
//! If no block's randomisation costs anything, that is the whole genome
//! answering at once, and the conclusion is to stop searching it.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::rng::Rng;

/// Named blocks of related genes, by index into `Weights::to_vec`.
const BLOCKS: [(&str, &[usize]); 8] = [
    ("expansion", &[0, 1, 2, 16]),
    ("economy", &[4, 17, 18, 19, 20, 21, 22]),
    ("opening", &[23, 24, 25, 26]),
    ("movement", &[27, 28]),
    ("doctrine", &[29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39]),
    ("combat_value", &[9, 10, 11]),
    ("war_decl", &[3, 5, 6, 7, 8]),
    ("policy", &[40, 41, 42, 43, 44, 45, 46, 47]),
];

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One mirrored map pair: the scrambled genome against the shipped one.
fn duel(
    candidate: &Weights,
    players: usize,
    w: i32,
    h: i32,
    seed: u64,
    turns: u32,
    lane: bool,
) -> f64 {
    let mut share = 0.0;
    for treated in 0..2usize {
        let mut game = Game::new(players, w, h, seed, turns, 0);
        let control = Weights::default();
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
            let value = if lane {
                game.victory_threat(player.id)
            } else {
                game.score(player.id) as f64
            };
            table += value;
            if is_treated(player.id) {
                mine += value;
            }
        }
        if lane {
            // Lane progress already tracks victory, so no win term is folded in;
            // mixing them would reintroduce the binary variance the continuous
            // statistic exists to avoid.
            share += if table > 0.0 { mine / table } else { 0.5 };
        } else {
            let won = if game.winner.is_some_and(is_treated) { 1.0 } else { 0.0 };
            share += 0.8 * (mine / table.max(1.0)) + 0.2 * won;
        }
    }
    share / 2.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 16);
    let draws = number(&args, "--draws", 3);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 1_100_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let lane = args.iter().any(|arg| arg == "--lane");

    let bounds = Weights::bounds();
    let names = Weights::gene_names();
    let shipped = Weights::default().to_vec();

    println!(
        "gene_leverage: {} blocks x {draws} random draws x {maps} mirrored maps, \
         {players}p {width}x{height}, {turns} turns, seed {seed0}",
        BLOCKS.len()
    );
    println!(
        "  statistic: {}",
        if lane { "victory-lane progress (tracks WINS)" } else { "score share (tracks the ECONOMY)" }
    );
    println!("  each draw replaces a block with uniform samples from its own bounds");
    println!("  parity 0.500; a block that matters scores BELOW parity when scrambled\n");

    // Score ONE gene value at whatever power the question deserves.
    //
    // A sweep nominates; this decides. `--sweep` walks N points on one seed
    // and its maximum is biased upward by construction, so the candidate it
    // nominates has to be re-measured alone, on maps it did not choose, at
    // enough of them to resolve the effect. Everything this session that
    // looked like a finding and was not — the opening book at +0.05, the
    // win-rate breeder, the expansion block — died at exactly this step.
    if let (Some(target), Some(value)) = (
        args.iter()
            .position(|arg| arg == "--at")
            .and_then(|index| args.get(index + 1)),
        args.iter()
            .position(|arg| arg == "--value")
            .and_then(|index| args.get(index + 1))
            .and_then(|v| v.parse::<f64>().ok()),
    ) {
        let Some(gene) = names.iter().position(|name| name == target) else {
            eprintln!("gene_leverage: no gene named {target:?}");
            std::process::exit(2);
        };
        let mut v = shipped.clone();
        v[gene] = value;
        let genome = Weights::from_vec(&v);
        let shares = parallel::map(maps, jobs, move |index| {
            duel(&genome, players, width, height, seed0 + index as u64, turns, lane)
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
        println!(
            "{target} = {value} (shipped {:.3}), {maps} mirrored maps, seed {seed0}",
            shipped[gene]
        );
        println!(
            "  {mean:.4} +/- {se:.4}   edge {edge:+.4}  ({:.1} SE)",
            if se > 0.0 { edge / se } else { 0.0 }
        );
        if se > 0.0 && edge.abs() < 2.0 * se {
            println!("  => inside the interval. Not distinguishable from the shipped value.");
        } else if edge > 0.0 {
            println!(
                "  => outside the interval and positive. Now decide it on WINS, not on score \
                 share."
            );
        }
        return;
    }

    // Enumerate every district PRIORITY ORDER.
    //
    // `economy` is the only load-bearing block in the genome (+0.0193 +/-
    // 0.0060, 3.2 SE), and the district priorities are its interesting half:
    // they decide which district a city builds first, which is the build-order
    // question. What matters about `d_campus`/`d_commercial`/`d_holy`/
    // `d_theater` is their RANKING, not their magnitudes -- the AI sorts by
    // them -- so the meaningful space is 4! = 24 orders, not a four-
    // dimensional continuum.
    //
    // Small and discrete, so enumerate it. Coordinate descent over continuous
    // values would re-run the opening book's selection bias for no reason.
    if args.iter().any(|arg| arg == "--districts") {
        let labels = ["campus", "commercial", "holy", "theater"];
        let slots = [19usize, 20, 21, 22];
        let shipped_order: Vec<f64> = slots.iter().map(|g| shipped[*g]).collect();
        println!(
            "enumerating all 24 district orders; shipped is campus {:.0} > commercial {:.0} > \
             holy {:.0} > theater {:.0}\n",
            shipped_order[0], shipped_order[1], shipped_order[2], shipped_order[3]
        );
        let mut perms: Vec<[usize; 4]> = Vec::new();
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let p = [a, b, c, d];
                        let mut seen = [false; 4];
                        if p.iter().all(|x| {
                            let fresh = !seen[*x];
                            seen[*x] = true;
                            fresh
                        }) {
                            perms.push(p);
                        }
                    }
                }
            }
        }
        let mut table: Vec<(f64, f64, String)> = Vec::new();
        for perm in &perms {
            let mut v = shipped.clone();
            // rank 0 is built first, so it takes the highest weight.
            for (slot_index, rank) in perm.iter().enumerate() {
                v[slots[slot_index]] = (4 - rank) as f64;
            }
            let genome = Weights::from_vec(&v);
            let shares = parallel::map(maps, jobs, move |index| {
                duel(&genome, players, width, height, seed0 + index as u64, turns, lane)
            });
            let n = shares.len().max(1) as f64;
            let mean = shares.iter().sum::<f64>() / n;
            let variance = if shares.len() > 1 {
                shares.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0)
            } else {
                0.0
            };
            let se = (variance / n).sqrt();
            let mut order: Vec<(usize, &str)> =
                perm.iter().copied().zip(labels).collect();
            order.sort_by_key(|(rank, _)| *rank);
            let name = order
                .iter()
                .map(|(_, label)| *label)
                .collect::<Vec<_>>()
                .join(" > ");
            println!("  {name:<44} {mean:.4} +/- {se:.4}   {:+.4}", mean - 0.5);
            table.push((mean, se, name));
        }
        table.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        println!("\nbest three:");
        for (mean, se, name) in table.iter().take(3) {
            println!("  {name:<44} {mean:.4} +/- {se:.4}");
        }
        println!(
            "\nTaking the max of 24 noisy cells is worth about +2 SE by construction even if\n\
             every order is identical. Re-measure the winner on disjoint maps before believing\n\
             it, then decide it on WINS."
        );
        return;
    }

    // Sweep one named gene across its own bounds, with an interval on each
    // point. This is the follow-up a load-bearing (or a scrambling-helped)
    // block earns: the block ablation says *whether* values matter, and only a
    // sweep says *which* value. Kept in the same binary because it shares the
    // paired duel and the same fitness.
    if let Some(target) = args
        .iter()
        .position(|arg| arg == "--sweep")
        .and_then(|index| args.get(index + 1))
    {
        let Some(gene) = names.iter().position(|name| name == target) else {
            eprintln!("gene_leverage: no gene named {target:?}");
            std::process::exit(2);
        };
        let (lo, hi) = bounds[gene];
        let points = number(&args, "--points", 7).max(2);
        println!("sweeping {target} over [{lo}, {hi}], shipped {:.3}\n", shipped[gene]);
        for step in 0..points {
            let value = lo + (hi - lo) * step as f64 / (points - 1) as f64;
            let mut v = shipped.clone();
            v[gene] = value;
            let genome = Weights::from_vec(&v);
            let shares = parallel::map(maps, jobs, move |index| {
                duel(&genome, players, width, height, seed0 + index as u64, turns, lane)
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
            let flag = if se > 0.0 && edge.abs() > 2.0 * se { "  <-- outside the interval" } else { "" };
            println!("  {value:>8.3}   {mean:.4} +/- {se:.4}   {edge:+.4}{flag}");
        }
        println!(
            "\nA point outside its interval nominates a value; it does not promote one.\n\
             Sweeping N points and taking the max is the same selection bias that made the\n\
             opening book look +0.05 before a disjoint holdout put it at -0.002. Re-measure\n\
             any winner on fresh maps, then decide it on WINS."
        );
        return;
    }

    let mut rows: Vec<(f64, f64, &str, usize)> = Vec::new();
    for (block_index, (block, genes)) in BLOCKS.iter().enumerate() {
        let mut per_draw: Vec<f64> = Vec::new();
        for draw in 0..draws {
            // Deterministic per (block, draw) so a rerun reproduces exactly.
            let mut rng = Rng::new(seed0 ^ ((block_index as u64) << 32) ^ draw as u64);
            let mut v = shipped.clone();
            for gene in genes.iter() {
                let (lo, hi) = bounds[*gene];
                v[*gene] = rng.uniform(lo, hi);
            }
            let genome = Weights::from_vec(&v);
            let map_seed = seed0 + 1_000 * (draw as u64 + 1);
            let shares = parallel::map(maps, jobs, move |index| {
                duel(&genome, players, width, height, map_seed + index as u64, turns, lane)
            });
            per_draw.push(shares.iter().sum::<f64>() / shares.len().max(1) as f64);
        }
        let n = per_draw.len().max(1) as f64;
        let mean = per_draw.iter().sum::<f64>() / n;
        let variance = if per_draw.len() > 1 {
            per_draw.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let se = (variance / n).sqrt();
        println!(
            "  {block:<14} {mean:.4} +/- {se:.4}   cost {:+.4}   [{}]",
            mean - 0.5,
            genes
                .iter()
                .map(|g| names[*g])
                .collect::<Vec<_>>()
                .join(", ")
        );
        rows.push((mean, se, block, genes.len()));
    }

    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("\nblocks ranked by what scrambling them costs (most damaging first):");
    for (mean, se, block, count) in &rows {
        let cost = 0.5 - mean;
        // Three outcomes, and the sign matters as much as the size. A negative
        // cost means scrambling HELPED, which is not the same as settled even
        // when it sits inside its interval -- it points the other way, at
        // shipped values that may be wrong rather than merely unimportant.
        let verdict = if cost < 0.0 && *se > 0.0 && -cost > 2.0 * se {
            "scrambling HELPED -- the shipped values are actively wrong"
        } else if cost < 0.0 {
            "scrambling helped, inside the interval -- a lead, sweep the genes"
        } else if *se > 0.0 && cost < 2.0 * se {
            "settled -- shipped values are not load-bearing"
        } else {
            "LOAD-BEARING -- better values could exist here"
        };
        println!("  {block:<14} cost {cost:+.4} +/- {se:.4}  ({count} genes)  {verdict}");
    }

    let any = rows
        .iter()
        .any(|(mean, se, _, _)| *se > 0.0 && (0.5 - mean) > 2.0 * se);
    if !any {
        println!(
            "\nNo block's randomisation costs anything outside its interval. Read that as the\n\
             whole genome answering at once: if getting a block WRONG is free, getting it\n\
             righter cannot pay, and a search over these genes is not the way to a stronger\n\
             agent. Every promoted gain in this repository came from more rollout instead."
        );
    }
}
