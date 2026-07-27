//! What are technology order and civic order worth?
//!
//! `docs/GENOME.md` establishes the discipline that produced every useful
//! result in this line of work: **bound a subsystem by ablation before
//! optimising inside it**. A null on selection is uninterpretable without the
//! ceiling beside it — that is what turned "policy cards don't matter" into
//! "the incumbent twenty-card list already captures the layer".
//!
//! Technology order and civic order have never been bounded, and they are two
//! of the decision layers the genome cannot reach at all: both are chosen by
//! hand-written code with no gene exposure, so evolution could not touch them
//! even if it worked.
//!
//! This measures them the cheapest honest way — by **taking them away**. After
//! the agent picks, the treated seats' choice is overwritten with a uniformly
//! random *legal* one. The engine validates the substitute, so the seat still
//! plays a legal game; it simply loses whatever the ordering heuristic knew.
//!
//! Scored on **wins**, paired and seat-mirrored, because everything in
//! `docs/GENOME.md` says a summary statistic is a correlate that breaks the
//! moment anything optimises against it. Nothing optimises here — this is an
//! ablation, not a search — but the same statistic is used so the number is
//! comparable with the rest.
//!
//! ```text
//! order_ablate --what tech --maps 60
//! order_ablate --what civic --maps 60
//! order_ablate --what both --maps 60
//! ```
//!
//! **Reading the result.** A large cost means the ordering heuristic is
//! carrying real weight and better orderings could exist — the only condition
//! under which work there can pay. A small cost means the layer is settled and
//! no amount of tech-order cleverness will make this agent stronger, which is
//! worth knowing before anyone writes one.
//!
//! The control arm is the stock agent, so parity is 0.500 and a *negative*
//! edge is the expected direction: randomising a decision should not help.
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

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// Replace this seat's current research and/or civic with a random legal one.
///
/// Applied *after* the agent's turn, so the heuristic runs and is then
/// overruled. `Action::Research` and `Action::Civic` go through the engine's
/// own validation, so an illegal substitute is simply refused and the seat
/// keeps what the agent chose — the ablation can only ever remove information,
/// never grant something the rules forbid.
fn scramble(game: &mut Game, pid: usize, rng: &mut Rng, tech: bool, civic: bool) {
    if tech {
        let available = game.available_techs(pid);
        if !available.is_empty() {
            let pick = available[rng.below(available.len())].clone();
            let _ = game.apply(pid, &Action::Research { tech: pick });
        }
    }
    if civic {
        let available = game.available_civics(pid);
        if !available.is_empty() {
            let pick = available[rng.below(available.len())].clone();
            let _ = game.apply(pid, &Action::Civic { civic: pick });
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 60);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 2_800_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let what = text(&args, "--what", "tech");
    let (tech, civic) = match what.as_str() {
        "tech" => (true, false),
        "civic" => (false, true),
        "both" => (true, true),
        other => {
            eprintln!("order_ablate: --what wants tech, civic or both; got {other:?}");
            std::process::exit(2);
        }
    };

    println!(
        "order_ablate: scrambling {what} order on the treated seats, {maps} mirrored maps, \
         {players}p {width}x{height}, {turns} turns, seed {seed0}"
    );
    println!("  scored on WINS; parity 0.500; a negative edge is the expected direction\n");

    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut out = [None, None];
        for (slot, treated) in (0..2usize).enumerate() {
            let mut game = Game::new(players, width, height, seed, turns, 0);
            let stock = Weights::default();
            let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
            // Seeded off the map and the direction so a rerun reproduces, and
            // so the two directions do not share a scramble sequence.
            let mut rng = Rng::new(seed ^ ((treated as u64 + 1) << 40) ^ 0x0BAD_5EED);
            let is_treated = |pid: usize| pid % 2 == treated;
            for _ in 0..turns {
                if game.winner.is_some() {
                    break;
                }
                for pid in 0..game.players.len() {
                    if game.winner.is_some() {
                        break;
                    }
                    fleet[pid].take_turn(&mut game, pid);
                    if !game.players[pid].is_minor && is_treated(pid) {
                        scramble(&mut game, pid, &mut rng, tech, civic);
                    }
                    if game.winner.is_none() && game.current == pid {
                        let _ = game.apply(pid, &Action::EndTurn);
                    }
                }
            }
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
        "\n  {}",
        if down > up && p < 0.05 {
            "COSTLY -- the ordering heuristic carries real weight, so better orderings could \
             exist and work there can pay."
        } else if n == 0 {
            "no map broke; raise --maps."
        } else {
            "SETTLED at this power -- scrambling the order costs nothing measurable, so no \
             amount of ordering cleverness will make this agent stronger."
        }
    );
    println!(
        "  Resolution: {maps} maps resolve about {:.3} of win rate, so a cost smaller than \
         roughly {:.2} is invisible here.",
        (0.25f64 / (maps as f64 * 2.0)).sqrt(),
        2.0 * (0.25f64 / (maps as f64 * 2.0)).sqrt()
    );
}
