//! Which genes can change a game at all?
//!
//! You cannot breed what does not bite. `civvis evolve` searches 40 genes, and
//! about a thousand rounds of live evolution produced no measurable gain
//! (`docs/RATING.md`). Two explanations have been offered — selection had no
//! signal, and the genome does not span the decisions — but a third has only
//! ever been *asserted*: that many genes have nothing to bite on. The war
//! genes are the standing example, since the AI takes 0.33 cities a game and
//! declares no peace ever, so `war_ratio`, `attack_floor`, `kill_bonus`,
//! `focus_fire`, `screen`, `cohesion` and friends may steer a subsystem that
//! never resolves.
//!
//! This measures it. For each gene, one seat plays with the shipped value and
//! then with a perturbed one, on the **same map against the same opponents**,
//! and the two games are compared turn by turn. A gene that never moves a
//! single observable has been shown, causally, not to affect those sampled
//! trajectories. That makes it low-priority in this search space, not globally
//! inert — the genome analogue of `search_probe`'s bounded `INERT` exit.
//!
//! ```text
//! gene_probe --maps 6 --turns 200
//! ```
//!
//! **The screen is one-directional, and that matters.** Divergence *proves* a
//! gene bites. Silence does not prove it is inert: a gene that only acts in
//! games that reach a war, or a city count, or an era that these few maps
//! never reach, will read silent for want of an occasion. So the report says
//! "no divergence in N maps", never "inert", and the honest use is to rank
//! genes by how hard they are to make bite — then either give the quiet ones
//! something to bite on, or stop paying for them in the search.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// What one seat looks like at the end of one turn. Deliberately coarse and
/// cheap: this asks whether the gene changed anything the game can see, not
/// how much.
#[derive(Clone, PartialEq)]
struct Snapshot {
    cities: usize,
    units: usize,
    techs: usize,
    civics: usize,
    policies: usize,
    score: i64,
    gold: i64,
    at_war: usize,
}

fn snapshot(g: &Game, pid: usize) -> Snapshot {
    let p = &g.players[pid];
    Snapshot {
        cities: g.player_city_ids(pid).len(),
        units: g.player_unit_ids(pid).len(),
        techs: p.techs.len(),
        civics: p.civics.len(),
        policies: p.policies.len(),
        score: g.score(pid),
        gold: p.gold as i64,
        at_war: g
            .players
            .iter()
            .filter(|q| !q.is_minor && q.id != pid && g.is_at_war(pid, q.id))
            .count(),
    }
}

/// Play one game with `seat` carrying `genome`, and record that seat each turn.
fn trace(genome: &Weights, players: usize, w: i32, h: i32, seed: u64, turns: u32, seat: usize) -> Vec<Snapshot> {
    let mut game = Game::new(players, w, h, seed, turns, 0);
    let stock = Weights::default();
    let mut mine: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, genome);
    let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
    let mut out = Vec::with_capacity(turns as usize);
    for _ in 0..turns {
        if game.winner.is_some() {
            break;
        }
        for pid in 0..game.players.len() {
            if game.winner.is_some() {
                break;
            }
            if pid == seat {
                mine[pid].take_turn(&mut game, pid);
            } else {
                rivals[pid].take_turn(&mut game, pid);
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        out.push(snapshot(&game, seat));
    }
    out
}

/// First turn on which two traces disagree, if they ever do.
fn divergence(a: &[Snapshot], b: &[Snapshot]) -> Option<usize> {
    let n = a.len().min(b.len());
    (0..n).find(|i| a[*i] != b[*i]).or({
        if a.len() != b.len() {
            Some(n)
        } else {
            None
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 6);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let seed0 = number(&args, "--seed", 700_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let seat = number(&args, "--seat", 0);

    let bounds = Weights::bounds();
    let stock = Weights::default().to_vec();
    let names = Weights::gene_names();
    let genes = stock.len();

    println!(
        "gene_probe: {genes} genes x {maps} maps, {players}p {width}x{height}, {turns} turns, \
         seat {seat}, seed {seed0}"
    );
    println!("  each gene is moved to both ends of its own bounds and compared with the shipped");
    println!("  value on the same map against the same opponents\n");

    // One baseline trace per map, shared by every gene.
    let baselines: Vec<Vec<Snapshot>> = parallel::map(maps, jobs, move |index| {
        trace(
            &Weights::default(),
            players,
            width,
            height,
            seed0 + index as u64,
            turns,
            seat,
        )
    });

    // A full sweep is fifty genes of full games. Verifying that one newly
    // connected gene bites should not cost that, so `--only <substring>`
    // narrows it -- the difference between a forty-minute answer and a
    // one-minute one, which is the difference between checking and not.
    let only = args
        .iter()
        .position(|arg| arg == "--only")
        .and_then(|index| args.get(index + 1))
        .cloned();
    if let Some(pattern) = &only {
        println!("  restricted to genes matching {pattern:?}\n");
    }

    let mut rows: Vec<(usize, String, usize, Option<f64>)> = Vec::new();
    for gene in 0..genes {
        if only
            .as_ref()
            .is_some_and(|pattern| !names[gene].contains(pattern.as_str()))
        {
            continue;
        }
        let (lo, hi) = bounds[gene];
        // Both ends, because a gene at its default may already sit against one
        // of its own bounds and moving that way would be a no-op by arithmetic.
        let probes: Vec<f64> = vec![lo, hi]
            .into_iter()
            .filter(|value| (value - stock[gene]).abs() > 1e-9)
            .collect();
        let mut bit = 0usize;
        let mut first: Vec<f64> = Vec::new();
        for value in probes {
            let mut v = stock.clone();
            v[gene] = value;
            let genome = Weights::from_vec(&v);
            let base = baselines.clone();
            let diverged = parallel::map(maps, jobs, move |index| {
                let other = trace(
                    &genome,
                    players,
                    width,
                    height,
                    seed0 + index as u64,
                    turns,
                    seat,
                );
                divergence(&base[index], &other)
            });
            for turn in diverged.into_iter().flatten() {
                bit += 1;
                first.push(turn as f64);
            }
        }
        let mean_turn = if first.is_empty() {
            None
        } else {
            Some(first.iter().sum::<f64>() / first.len() as f64)
        };
        rows.push((gene, names[gene].to_string(), bit, mean_turn));
    }

    rows.sort_by_key(|r| r.2);
    let quiet: Vec<&(usize, String, usize, Option<f64>)> =
        rows.iter().filter(|r| r.2 == 0).collect();

    println!("{:<4} {:<20} {:>8}  {}", "idx", "gene", "bites", "mean first divergence turn");
    for (index, name, bit, mean) in &rows {
        let when = mean
            .map(|t| format!("{t:.0}"))
            .unwrap_or_else(|| "-".to_string());
        println!("{index:<4} {name:<20} {bit:>3}/{:<4} {when}", 2 * maps);
    }

    println!(
        "\n{} of {} genes probed moved nothing in {maps} maps at {turns} turns.",
        quiet.len(),
        rows.len()
    );
    if !quiet.is_empty() {
        println!(
            "  quiet: {}",
            quiet
                .iter()
                .map(|r| r.1.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "  Silence is not proof of inertness -- a gene that only acts in a game reaching a\n  \
             war, a city count or an era these maps never reach reads quiet for want of an\n  \
             occasion. Raise --maps and --turns before concluding anything about one."
        );
    }
}
