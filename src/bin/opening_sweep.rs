//! Sweep the opening book, which is the strongest live block in the genome.
//!
//! `gene_probe` ranked every gene by how readily it changes a game. The
//! opening book took the top of that table — `open0` diverges in **12 of 12**
//! trials with a mean first divergence at **turn 8**, earlier and harder than
//! anything else in the genome, and `open1`/`open2` follow at 11/12 and 10/12.
//! Meanwhile eleven genes moved nothing at all. So this is where a search has
//! the most to bite on, and nobody has ever swept it.
//!
//! It is also the rare corner of this genome that is **small and discrete**.
//! Four capital builds, each an index into a six-entry `OPENING_MENU` plus a
//! seventh "no scripted pick, evaluate normally" — 7^4 = 2401 books. The
//! policy appetites were an eight-dimensional continuum where a GA had to
//! guess; here coordinate descent over 28 cells covers every option for every
//! slot, and the answer is a table rather than a champion.
//!
//! Shipped book: warrior, settler, builder, monument.
//!
//! ```text
//! opening_sweep --maps 16 --players 4
//! ```
//!
//! **Every cell carries a standard error, and the assembled best is re-checked
//! on disjoint maps.** Coordinate descent picks four maxima out of 28 noisy
//! cells, so the winner is biased upward by selection even if no slot matters
//! at all — the holdout is what says whether any of it was real. Promotion
//! still needs a pre-registered `policy_eval`-style run on wins.
use civvis::ai::{AdvancedAi, Ai, Weights, OPENING_MENU};
use civvis::game::{Action, Game};
use civvis::parallel;

/// Genome indices of the four opening-book slots, in build order.
const BOOK_GENES: [usize; 4] = [23, 24, 25, 26];
/// Menu length plus one for "no scripted pick".
const OPTIONS: usize = OPENING_MENU.len() + 1;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn option_name(value: usize) -> &'static str {
    OPENING_MENU.get(value).copied().unwrap_or("(evaluate)")
}

/// One mirrored map pair: the candidate book against the shipped one.
fn duel(candidate: &Weights, players: usize, w: i32, h: i32, seed: u64, turns: u32) -> f64 {
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
            let score = game.score(player.id) as f64;
            table += score;
            if is_treated(player.id) {
                mine += score;
            }
        }
        let won = if game.winner.is_some_and(is_treated) { 1.0 } else { 0.0 };
        share += 0.8 * (mine / table.max(1.0)) + 0.2 * won;
    }
    share / 2.0
}

/// Mean share and the standard error of that mean.
fn score(
    book: &[usize; 4],
    players: usize,
    w: i32,
    h: i32,
    maps: usize,
    seed0: u64,
    turns: u32,
    jobs: usize,
) -> (f64, f64) {
    let mut v = Weights::default().to_vec();
    for (slot, value) in BOOK_GENES.iter().zip(book) {
        v[*slot] = *value as f64;
    }
    let genome = Weights::from_vec(&v);
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 16);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 900_000) as u64;
    let holdout_maps = number(&args, "--holdout-maps", 48);
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    let shipped_vec = Weights::default().to_vec();
    let shipped: [usize; 4] = [
        shipped_vec[BOOK_GENES[0]] as usize,
        shipped_vec[BOOK_GENES[1]] as usize,
        shipped_vec[BOOK_GENES[2]] as usize,
        shipped_vec[BOOK_GENES[3]] as usize,
    ];

    println!(
        "opening_sweep: 4 slots x {OPTIONS} options, {maps} mirrored maps each, \
         {players}p {width}x{height}, {turns} turns, seed {seed0}"
    );
    println!(
        "  shipped book: {}",
        shipped
            .iter()
            .map(|v| option_name(*v))
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    println!("  fitness 0.8*score share + 0.2*win rate against that book; parity 0.500\n");

    let mut best = shipped;
    // What the book carried into this slot scored when it was selected, so the
    // next slot's sweep -- which runs on fresh maps -- doubles as a replication
    // test of the previous pick. This was originally an accident of varying the
    // seed per slot, and it caught the first winner: `slinger` scored 0.5303 on
    // the maps that chose it and 0.4923 on the next set. Reporting it is worth
    // more than the four cells it costs, because a coordinate descent that
    // cannot see its own picks failing will happily stack four of them.
    let mut carried: Option<(f64, f64, usize)> = None;
    for (slot, gene) in BOOK_GENES.iter().enumerate() {
        let _ = gene;
        println!("slot {slot}:");
        let slot_seed = seed0 + 100 * slot as u64;
        let mut rows: Vec<(f64, f64, usize)> = Vec::new();
        for value in 0..OPTIONS {
            let mut book = best;
            book[slot] = value;
            let (mean, se) = score(&book, players, width, height, maps, slot_seed, turns, jobs);
            let mut marker = String::new();
            if value == shipped[slot] {
                marker.push_str(" (shipped)");
            }
            // The carried book reappears here as the row that leaves this slot
            // at its incoming value.
            if let Some((was, was_se, from_slot)) = carried {
                if value == best[slot] {
                    let delta = mean - was;
                    let delta_se = (se * se + was_se * was_se).sqrt();
                    marker.push_str(&format!(
                        " (carried from slot {from_slot}: scored {was:.4} there, {delta:+.4} +/- {delta_se:.4} here)"
                    ));
                }
            }
            println!("  {:<12} {mean:.4} +/- {se:.4}{marker}", option_name(value));
            rows.push((mean, se, value));
        }
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let (won, won_se, value) = rows[0];
        best[slot] = value;
        carried = Some((won, won_se, slot));
        println!(
            "  -> keeping {} for slot {slot} at {won:.4} +/- {won_se:.4}\n",
            option_name(best[slot])
        );
    }

    println!(
        "assembled book: {}",
        best.iter()
            .map(|v| option_name(*v))
            .collect::<Vec<_>>()
            .join(" -> ")
    );

    if best == shipped {
        println!("  identical to the shipped book -- the sweep found nothing to change.");
        return;
    }

    // Coordinate descent takes four maxima out of 28 noisy cells, so the
    // assembled book is biased upward by selection even if no slot matters.
    // Disjoint maps are what separate that from a real effect.
    let holdout_seed = seed0 + 500_000;
    let (mine, mine_se) = score(
        &best, players, width, height, holdout_maps, holdout_seed, turns, jobs,
    );
    let edge = mine - 0.5;
    println!("\nholdout, {holdout_maps} disjoint maps at seed {holdout_seed}:");
    println!("  assembled vs shipped  {mine:.4} +/- {mine_se:.4}");
    println!(
        "  edge over parity      {edge:+.4} ({:.1} SE)",
        if mine_se > 0.0 { edge / mine_se } else { 0.0 }
    );
    if mine_se > 0.0 && edge.abs() < 2.0 * mine_se {
        println!(
            "  => inside the interval. The sweep's winner does not survive maps it did not\n     \
             select on; this is selection bias, not an opening. Do not promote."
        );
    } else if edge > 0.0 {
        println!(
            "  => outside the interval and positive. Earns a pre-registered run on WINS at\n     \
             100+ mirrored maps, which decides it -- this number only nominates."
        );
    }
}
