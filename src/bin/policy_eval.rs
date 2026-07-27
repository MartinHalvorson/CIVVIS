//! Decide the policy-deck change on mirrored maps.
//!
//! `ai_eval` takes two entrant *names* and resolves them through
//! `elo::builtin_ai`, so an A/B between two configurations of the same agent
//! needs a registered entrant, and `src/elo.rs` belongs to another open PR.
//! The switch this harness needs already travels on `Weights`
//! (`policy_deck`, deliberately not a gene), so both arms can be built here
//! from `AdvancedAi::fleet_weighted` and nothing outside `src/ai.rs` has to
//! move.
//!
//! Three arms, and the third is the important one. `--treatment legacy
//! --control empty` slots no cards at all on one side, which measures what the
//! entire policy layer is worth and therefore **bounds what any card policy
//! can win**. Run that before optimising within a subsystem, not after: `live`
//! against `legacy` came back a clean null (18 for / 15 against, p=0.7283 over
//! 120 maps) and that number is uninterpretable without the ceiling beside it.
//!
//! The design is the paired one the repository decides on. Each map is played
//! twice: once with the treatment on the even seats, once on the odd seats. A
//! map counts **for** the treatment only when it wins both directions and
//! **against** only when it loses both; a split is neutral and carries no
//! information about the treatment, because the two arms met on the same
//! ground from both sides.
//!
//! ## Why "wins both halves" is unbiased *here*
//!
//! PR #366 found that scoring a map by whether a granted seat won both
//! mirrored halves is an artifact generator: it grants **one** seat of four,
//! so under the null P(wins both) is about 0.06 against P(neither) about 0.56,
//! and a control arm dutifully reported p=0.0000 having measured nothing.
//!
//! This harness is not exposed to that, because the two arms are a symmetric
//! **partition** of the table rather than one seat against three. Two treated
//! seats of four, and the arms swap sides between directions. Write `q` for
//! the chance the even-seat pair wins under the null; then the treatment takes
//! direction 0 with probability `q` and direction 1 with probability `1-q`, so
//!
//! - P(treatment wins both) = `q(1-q)`
//! - P(control wins both) = `(1-q)q`
//!
//! — identical, for every `q`. So the sign test is valid **even though the civ
//! effect is large** (`docs/RATING.md`: Rome takes 37.7% of wins, Sumeria
//! 14.6%) and even though civs are fixed per seat index. The mirroring cancels
//! it exactly rather than averaging over it.
//!
//! That argument is checked empirically, not just asserted: run
//! `--treatment legacy --control legacy`. Identical arms must land near parity
//! with a non-significant sign test. If they do not, every number this harness
//! has produced is its own noise.
//!
//! ```text
//! policy_eval --players 4 --maps 120
//! policy_eval --players 4 --maps 120 --treatment legacy --control empty
//! ```
//!
//! Read the map-direction line and the sign test, not the raw game count:
//! `docs/EVAL.md` records two conclusions from this evaluator that inverted at
//! 120 maps in opposite directions, and 20-map runs on it are anti-evidence.
use civvis::ai::{AdvancedAi, Ai, PolicyDeck, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn deck(name: &str) -> PolicyDeck {
    match name {
        "live" => PolicyDeck::Live,
        "legacy" => PolicyDeck::Legacy,
        "empty" => PolicyDeck::Empty,
        other => {
            eprintln!("policy_eval: unknown arm {other:?}; use live, legacy or empty");
            std::process::exit(2);
        }
    }
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Two-sided sign test over the maps that broke, by exact binomial.
fn sign_p(up: u32, down: u32) -> f64 {
    let n = up + down;
    if n == 0 {
        return 1.0;
    }
    let extreme = up.min(down);
    // sum_{k<=extreme} C(n,k) / 2^n, doubled, clamped.
    let mut log_c = 0.0f64;
    let mut tail = 0.0f64;
    for k in 0..=extreme {
        if k > 0 {
            log_c += ((n - k + 1) as f64).ln() - (k as f64).ln();
        }
        tail += (log_c - (n as f64) * std::f64::consts::LN_2).exp();
    }
    (2.0 * tail).min(1.0)
}

/// Play one map with the treatment on the seats whose parity is `treated`.
///
/// The win is an `Option`: a game that reached its turn cap with nobody
/// victorious says nothing about either arm, and folding that into "the
/// treatment did not win" would count silence as a defeat.
fn play(
    players: usize,
    width: i32,
    height: i32,
    seed: u64,
    turns: u32,
    treated: usize,
    arms: (PolicyDeck, PolicyDeck),
) -> (Option<bool>, f64, f64) {
    let mut game = Game::new(players, width, height, seed, turns, 0);
    let treat_w = Weights {
        policy_deck: arms.0,
        ..Weights::default()
    };
    let control_w = Weights {
        policy_deck: arms.1,
        ..Weights::default()
    };
    let mut treatment: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &treat_w);
    let mut control: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &control_w);
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
                control[pid].take_turn(&mut game, pid);
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
    }

    let won = game.winner.map(is_treated);
    let mut mine = 0.0;
    let mut table = 0.0;
    for player in game.players.iter().filter(|p| !p.is_minor) {
        let score = game.score(player.id) as f64;
        table += score;
        if is_treated(player.id) {
            mine += score;
        }
    }
    (won, mine, table)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 120);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    // Standard speed's stock budget. `docs/EVAL.md`: a short cap does not
    // shorten a game, it changes the answer -- at 250 turns most games end on
    // the cap as score victories and three victory types never occur at all.
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 300_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());

    let treat_name = text(&args, "--treatment", "live");
    let control_name = text(&args, "--control", "legacy");
    let arms = (deck(&treat_name), deck(&control_name));
    println!(
        "policy_eval: {treat_name} vs {control_name}, {maps} maps x 2 directions, \
         {players}p {width}x{height}, {turns} turns, seed {seed0}"
    );

    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let a = play(players, width, height, seed, turns, 0, arms);
        let b = play(players, width, height, seed, turns, 1, arms);
        (a, b)
    });

    let mut games_won = 0u32;
    let mut decisive = 0u32;
    let mut games = 0u32;
    let mut up = 0u32;
    let mut down = 0u32;
    let mut neutral = 0u32;
    let mut share_sum = 0.0f64;
    let mut share_n = 0.0f64;
    for (a, b) in &results {
        for arm in [a, b] {
            games += 1;
            match arm.0 {
                Some(true) => {
                    decisive += 1;
                    games_won += 1;
                }
                Some(false) => decisive += 1,
                None => {}
            }
            if arm.2 > 0.0 {
                // Two treated seats of four, so parity share is 0.5.
                share_sum += arm.1 / arm.2;
                share_n += 1.0;
            }
        }
        // Wins and terminal score measure different things, and `docs/EVAL.md`
        // records the cost of confusing them: a change that re-routes victory
        // lanes moves wins while score stays flat, which is the predicted
        // signature and not a contradiction. So the map direction is decided on
        // wins alone, and the score share is reported beside it, never folded in.
        match (a.0, b.0) {
            (Some(true), Some(true)) => up += 1,
            (Some(false), Some(false)) => down += 1,
            _ => neutral += 1,
        }
    }

    let p = sign_p(up, down);
    println!(
        "  decisive games   {games_won}/{decisive} ({:.1}%), {} of {games} reached a victory",
        100.0 * games_won as f64 / decisive.max(1) as f64,
        decisive
    );
    println!("  map directions   {up} for / {down} against / {neutral} neutral");
    println!("  sign test        p = {p:.4}");
    println!(
        "  terminal score   {:.1}% of table (parity is 50.0%)",
        100.0 * share_sum / share_n.max(1.0)
    );
    println!(
        "  resolution       directions rest on {} of {maps} maps that broke",
        up + down
    );
    if maps < 100 {
        println!(
            "  ⚠ under 100 maps this evaluator has inverted conclusions in both \
             directions; treat as a screen, not a verdict."
        );
    }
}
