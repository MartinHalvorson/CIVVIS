//! Does the macro search under-value Religion because it projects that lane
//! with an empire that has stopped expanding?
//!
//! `StrategicAi::branch_agent` builds every branch by calling `retarget`, and
//! `AdvancedAi::assess` sends an assigned-Religion seat that has no religion
//! yet straight to `GrandStrategy::Religion`, skipping the "can this lane still
//! afford to expand first?" test that every *other* assigned target reaches.
//! Measured end-to-end (`commit_curve`, `docs/EVAL.md` 2026-07-28), a seat
//! committed to Religion finishes on **1.68 cities against an adaptive seat's
//! 4.10**.
//!
//! So a religion branch is projected by an empire that stops growing while the
//! adaptive branch it is ranked against keeps growing. If that biases the
//! ranking it biases it against the lane this engine converts best.
//!
//! This is the screen, in the `search_probe` idiom: flip one flag on **one
//! agent at one position** and read the branch values the review actually
//! compares. It is **paired** by construction — same game, same seat, same
//! plan in force, same rollout budget, one boolean apart.
//!
//! Reported per position: the religion branch's projected value with the flag
//! off and on, whether the argmax lane changed, and in which direction. A
//! treatment that cannot move these numbers cannot move a win rate, so this
//! runs before any `ai_eval`.
//!
//! **Exits 3 on INERT** — every branch value identical on every position — for
//! the same reason `search_probe` does: that is not a null, it is a treatment
//! that never applied, and the two call for opposite next steps.
//!
//! ```text
//! lane_projection --maps 8 --players 4 --sample-turn 60
//! ```
//!
//! Diagnostic only: it never changes a shipped decision.
use civvis::ai::{Ai, VictoryTarget, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::strategic::StrategicAi;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// One sampled position: the religion branch off/on, and the lane each would
/// pick.
struct Position {
    religion_off: f64,
    religion_on: f64,
    best_off: Option<VictoryTarget>,
    best_on: Option<VictoryTarget>,
    spread_off: f64,
    spread_on: f64,
}

fn argmax(values: &[(f64, Option<VictoryTarget>)]) -> Option<VictoryTarget> {
    values
        .iter()
        .fold(None::<(f64, Option<VictoryTarget>)>, |best, (value, target)| {
            match best {
                Some((b, _)) if b >= *value => best,
                _ => Some((*value, *target)),
            }
        })
        .and_then(|(_, target)| target)
}

fn spread(values: &[(f64, Option<VictoryTarget>)]) -> f64 {
    let max = values.iter().map(|(v, _)| *v).fold(f64::NEG_INFINITY, f64::max);
    let min = values.iter().map(|(v, _)| *v).fold(f64::INFINITY, f64::min);
    max - min
}

fn religion_value(values: &[(f64, Option<VictoryTarget>)]) -> Option<f64> {
    values
        .iter()
        .find(|(_, target)| *target == Some(VictoryTarget::Religion))
        .map(|(value, _)| *value)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 8);
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let city_states = number(&args, "--city-states", 6);
    let turns = number(&args, "--turns", 500) as u32;
    let sample_turn = number(&args, "--sample-turn", 60) as u32;
    let seed0 = number(&args, "--seed", 1_900_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    // The 120-map eval said permitting expansion in a religion branch costs 53
    // Elo, and attributed it to a settler not paying back before the branch is
    // scored. That is a claim about the *window*: at a long enough horizon the
    // settler pays inside it and the sign should move. This is how to check
    // that in minutes instead of the four hours a `strategic_deep` eval costs.
    let horizon = number(&args, "--horizon", 40) as u32;

    println!(
        "lane_projection: {maps} maps, {players}p {width}x{height}, {city_states} city-states, \
         sampling at turn {sample_turn}, horizon {horizon}, seed {seed0}"
    );
    println!("paired: one agent, one position, one boolean apart\n");

    let sampled = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, city_states);
        let genome = civvis::evolve::load_champion("evolved").unwrap_or_default();
        // The focal seat runs the real search; the rest play the stock fleet
        // agent, exactly as a league game does.
        let mut probe = StrategicAi::with_weights(genome.clone());
        probe.horizon = horizon;
        let mut fleet = civvis::ai::AdvancedAi::fleet_weighted(&game, &genome);

        let focal = index % players;
        let mut out: Option<Position> = None;
        for turn in 0..turns {
            if game.winner.is_some() || turn > sample_turn {
                break;
            }
            if turn == sample_turn {
                // Both readings come off the same agent in the same state.
                probe.branch_religion_may_expand = false;
                let off = probe.lane_values(&game, focal);
                probe.branch_religion_may_expand = true;
                let on = probe.lane_values(&game, focal);
                if let (Some(religion_off), Some(religion_on)) =
                    (religion_value(&off), religion_value(&on))
                {
                    out = Some(Position {
                        religion_off,
                        religion_on,
                        best_off: argmax(&off),
                        best_on: argmax(&on),
                        spread_off: spread(&off),
                        spread_on: spread(&on),
                    });
                }
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if pid == focal {
                    probe.take_turn(&mut game, pid);
                } else {
                    fleet[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        out
    });

    let positions: Vec<Position> = sampled.into_iter().flatten().collect();
    if positions.is_empty() {
        println!("no position produced a religion branch; nothing to compare");
        std::process::exit(3);
    }

    let n = positions.len();
    let moved = positions
        .iter()
        .filter(|p| (p.religion_on - p.religion_off).abs() > 1e-9)
        .count();
    if moved == 0 {
        println!("{n} positions, religion branch value identical on every one");
        println!("\nINERT: the treatment never changed a number the review compares, so it \
                  cannot change a decision. This is not a null result — it is a treatment \
                  that did not apply. Check that the flag reaches `branch_agent` before \
                  reading anything else into it.");
        std::process::exit(3);
    }

    let up = positions.iter().filter(|p| p.religion_on > p.religion_off + 1e-9).count();
    let down = positions.iter().filter(|p| p.religion_on < p.religion_off - 1e-9).count();
    let flipped = positions.iter().filter(|p| p.best_off != p.best_on).count();
    let to_religion = positions
        .iter()
        .filter(|p| p.best_off != p.best_on && p.best_on == Some(VictoryTarget::Religion))
        .count();
    let from_religion = positions
        .iter()
        .filter(|p| p.best_off != p.best_on && p.best_off == Some(VictoryTarget::Religion))
        .count();
    let mean_delta: f64 = positions
        .iter()
        .map(|p| p.religion_on - p.religion_off)
        .sum::<f64>()
        / n as f64;
    let mean_spread_off: f64 = positions.iter().map(|p| p.spread_off).sum::<f64>() / n as f64;
    let mean_spread_on: f64 = positions.iter().map(|p| p.spread_on).sum::<f64>() / n as f64;

    println!("positions sampled       {n}");
    println!("religion value moved    {moved}  ({up} up / {down} down)");
    println!("mean change             {mean_delta:+.4}");
    println!("branch spread           {mean_spread_off:.4} -> {mean_spread_on:.4}");
    println!("argmax lane changed     {flipped}");
    println!("  toward religion       {to_religion}");
    println!("  away from religion    {from_religion}");

    // Branch on what was measured.
    println!();
    if flipped == 0 {
        println!(
            "READING: the projection moves but the DECISION does not. The religion branch \
             changed on {moved} of {n} positions and the argmax lane changed on none of \
             them, so the bias is real and smaller than the gaps between lanes. An eval \
             would be measuring nothing; find a position class where the lanes are closer \
             before spending one."
        );
    } else if to_religion > from_religion {
        println!(
            "READING: the search was under-selecting Religion. On {flipped} of {n} positions \
             the argmax lane changed, {to_religion} of them toward Religion against \
             {from_religion} away. The projection defect biases the ranking against the \
             lane this engine converts best. This is worth a pre-registered `ai_eval`."
        );
    } else {
        println!(
            "READING: the decision moves, but not toward Religion — {flipped} lane changes, \
             {to_religion} toward Religion and {from_religion} away. The projection defect \
             is real and its effect on routing is not the one predicted. Do not run an eval \
             on the predicted mechanism; work out what the flag is actually doing first."
        );
    }
}
