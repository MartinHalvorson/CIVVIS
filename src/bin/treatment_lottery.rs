//! The treatment lottery: every game draws a random subset of treatments to
//! withhold, and the average over many games prices every treatment at once.
//!
//! `docs/EVAL.md` prices behaviours by withholding one at a time, which is the
//! confirmation standard — but it spends a whole eval round per flag, and 24 of
//! the 55 headless-measurable treatments have never been named in any round.
//! This tool is the screening tier in front of that standard. Each game draws
//! an independent random withhold-vector over the factor set (every factor
//! withheld with probability `--density`), plays the drawn agent against a
//! same-seed full-bundle control on the same seat (the `gene_census` pairing),
//! and regresses the outcome delta on the vector by marginal averaging: a
//! factor's price is the mean delta of games where it was withheld minus the
//! mean delta of games where it was kept. Randomization makes the other
//! factors cancel in expectation, so one game contributes an observation to
//! every factor simultaneously — the whole reason this costs less than a
//! round per flag.
//!
//! What the number is, and is not:
//! - It is an unbiased estimate of the factor's average main effect over the
//!   mixture the lottery draws — at `--density 0.5` that is an agent missing
//!   about half its bundle, NOT the deployment point. The repo's own ledger
//!   shows components of one constructor at −41 and +30 Elo, so interactions
//!   are real here; a lottery signal licenses a single-flag confirmation arm
//!   (`live_without_<tag>` / a withholding round), never a ship decision.
//! - A null is bounded by the fires-check, same as everywhere else: a factor
//!   whose branch never executes at this profile prices at exactly zero, and
//!   that zero is indistinguishable from a real one. `moved` counts games
//!   containing the factor whose outcome differed at all — a ceiling, not an
//!   attribution, since other drawn factors also differ in those games.
//! - One seed range is never a result. Confirm any signal on a disjoint
//!   `--start-seed` before writing it down.
//!
//! Every game's draw and both outcomes go to a JSONL ledger (`--out`), which
//! is the artifact nothing else in the repo carries: a per-game flag vector
//! beside a per-game outcome, so the analysis can be redone or extended
//! without replaying anything.
//!
//! Usage: treatment_lottery [--games N] [--density P] [--draw-seed N]
//!                          [--start-seed N] [--players N] [--turns N]
//!                          [--width N] [--height N] [--city-states N]
//!                          [--factors tag,tag,...] [--jobs N] [--out PATH]
//!                          [--list]
use civvis::ai::{AdvancedAi, Ai, LiveTreatment, LIVE_TREATMENTS, PRODUCTION_TREATMENTS};
use civvis::elo::ENGINE_REPAIR_TREATMENTS;
use civvis::game::{Action, Game, GameOptions};
use civvis::rng::Rng;
use std::io::Write;

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn float(args: &[String], flag: &str, default: f64) -> f64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

/// The default factor set: every registered live treatment that repairs a
/// CIVVIS engine defect, i.e. can actually fire headless. The Firaxis-only
/// rows would price at exactly zero here and waste the whole design's power.
fn default_factors() -> Vec<&'static LiveTreatment> {
    LIVE_TREATMENTS
        .iter()
        .filter(|(_, tag, _)| ENGINE_REPAIR_TREATMENTS.contains(tag))
        .collect()
}

/// Resolve `--factors` names against the registries, matching either the
/// field name or the kebab tag the way `victory_eval --without` does. An
/// unknown name is a hard error: a typo that silently shrank the factor set
/// would report marginals for a design nobody chose.
fn resolve_factors(names: &str) -> Result<Vec<&'static LiveTreatment>, String> {
    let table = || LIVE_TREATMENTS.iter().chain(PRODUCTION_TREATMENTS.iter());
    names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            table()
                .find(|(field, tag, _)| *field == name || *tag == name)
                .ok_or_else(|| {
                    let known: Vec<&str> = table().map(|(_, tag, _)| *tag).collect();
                    format!(
                        "unknown treatment {name:?}; known tags: {}",
                        known.join(", ")
                    )
                })
        })
        .collect()
}

/// Draw the withheld subset for one game: factor `i` is withheld when the
/// game's own stream says so. Deterministic in (`draw_seed`, game offset), so
/// a ledger row can be reproduced without the ledger.
fn draw(draw_seed: u64, offset: u64, count: usize, density: f64) -> Vec<usize> {
    let mut rng = Rng::new(draw_seed.wrapping_add(offset));
    (0..count).filter(|_| rng.f64() < density).collect()
}

/// One game's result from the treated seat's perspective.
#[derive(PartialEq, Debug, Clone)]
struct Outcome {
    won: bool,
    score_share: f64,
    score: i64,
    turn: u32,
}

fn play(
    options: GameOptions,
    treated_seat: usize,
    withheld: &[usize],
    factors: &[&'static LiveTreatment],
) -> Outcome {
    let mut g = Game::new_with(options);
    let mut ais: Vec<Box<dyn Ai>> = (0..g.players.len())
        .map(|pid| {
            if pid == treated_seat {
                let mut ai = AdvancedAi::new();
                ai.enable_live_bridge();
                for &index in withheld {
                    (factors[index].2)(&mut ai);
                }
                Box::new(ai) as Box<dyn Ai>
            } else {
                Box::new(AdvancedAi::new()) as Box<dyn Ai>
            }
        })
        .collect();
    while g.winner.is_none() {
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    let scores: Vec<i64> = (0..g.players.len()).map(|pid| g.score(pid)).collect();
    let total: i64 = scores.iter().sum();
    Outcome {
        won: g.winner == Some(treated_seat),
        score_share: if total > 0 {
            scores[treated_seat] as f64 / total as f64
        } else {
            0.0
        },
        score: scores[treated_seat],
        turn: g.reported_turn(),
    }
}

/// One factor's marginal price over the whole batch.
struct Contrast {
    n_off: usize,
    n_on: usize,
    /// mean(delta | withheld) − mean(delta | kept); negative means withholding
    /// hurt, i.e. the treatment is an asset.
    contrast: f64,
    se: f64,
    win_contrast: f64,
}

fn mean_var(values: &[f64]) -> (f64, f64) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    (mean, var)
}

/// Marginal averaging over (withheld set, score delta, win delta) rows. Split
/// out from `main` so the estimator can be tested on planted effects without
/// playing a game.
fn contrasts(rows: &[(Vec<usize>, f64, f64)], count: usize) -> Vec<Contrast> {
    (0..count)
        .map(|factor| {
            let (mut off, mut on, mut win_off, mut win_on) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for (withheld, delta, win_delta) in rows {
                if withheld.contains(&factor) {
                    off.push(*delta);
                    win_off.push(*win_delta);
                } else {
                    on.push(*delta);
                    win_on.push(*win_delta);
                }
            }
            let (m_off, v_off) = mean_var(&off);
            let (m_on, v_on) = mean_var(&on);
            let (w_off, _) = mean_var(&win_off);
            let (w_on, _) = mean_var(&win_on);
            let se = if off.len() > 1 && on.len() > 1 {
                (v_off / off.len() as f64 + v_on / on.len() as f64).sqrt()
            } else {
                f64::INFINITY
            };
            Contrast {
                n_off: off.len(),
                n_on: on.len(),
                contrast: m_off - m_on,
                se,
                win_contrast: w_off - w_on,
            }
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 40).max(1) as u64;
    let density = float(&args, "--density", 0.5).clamp(0.0, 1.0);
    let draw_seed = number(&args, "--draw-seed", 11_000) as u64;
    let start = number(&args, "--start-seed", 990_000) as u64;
    let players = number(&args, "--players", 4).max(2) as usize;
    let turns = number(&args, "--turns", 220).max(1) as u32;
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let city_states = number(&args, "--city-states", 6) as usize;
    let jobs = number(&args, "--jobs", civvis::parallel::default_jobs() as i64).max(1) as usize;

    let factors: Vec<&'static LiveTreatment> = match text(&args, "--factors") {
        Some(names) => match resolve_factors(&names) {
            Ok(rows) => rows,
            Err(why) => {
                eprintln!("treatment-lottery: {why}");
                std::process::exit(2);
            }
        },
        None => default_factors(),
    };
    if args.iter().any(|arg| arg == "--list") {
        for (field, tag, _) in &factors {
            println!("{tag}  ({field})");
        }
        return;
    }
    let out_path = text(&args, "--out")
        .unwrap_or_else(|| format!("lottery-s{start}-d{draw_seed}-g{games}.jsonl"));

    println!(
        "treatment lottery: {games} draws x {players}p {width}x{height}, {turns} turns, \
         seeds {start}..{}, density {density}, draw seed {draw_seed}",
        start + games - 1
    );
    println!(
        "{} factors; each draw plays the drawn agent and a same-seed full-bundle \
         control on the same seat; ledger -> {out_path}\n",
        factors.len()
    );

    let records = civvis::parallel::map(games as usize, jobs, |offset| {
        let seed = start + offset as u64;
        let withheld = draw(draw_seed, offset as u64, factors.len(), density);
        // Rotate the treated seat so the finding is not one seat's quirk.
        let treated = (seed as usize) % players;
        let options = || GameOptions::new(players, width, height, seed, turns, city_states);
        let lottery = play(options(), treated, &withheld, &factors);
        let control = play(options(), treated, &[], &factors);
        (seed, treated, withheld, lottery, control)
    });

    let mut ledger = std::fs::File::create(&out_path)
        .unwrap_or_else(|why| panic!("cannot create {out_path}: {why}"));
    for (seed, treated, withheld, lottery, control) in &records {
        let tags: Vec<&str> = withheld.iter().map(|&i| factors[i].1).collect();
        let row = serde_json::json!({
            "seed": seed,
            "draw_seed": draw_seed,
            "density": density,
            "treated_seat": treated,
            "withheld": tags,
            "lottery": {"won": lottery.won, "score_share": lottery.score_share,
                        "score": lottery.score, "turn": lottery.turn},
            "control": {"won": control.won, "score_share": control.score_share,
                        "score": control.score, "turn": control.turn},
        });
        writeln!(ledger, "{row}").expect("ledger write failed");
    }

    let rows: Vec<(Vec<usize>, f64, f64)> = records
        .iter()
        .map(|(_, _, withheld, lottery, control)| {
            (
                withheld.clone(),
                lottery.score_share - control.score_share,
                lottery.won as i64 as f64 - control.won as i64 as f64,
            )
        })
        .collect();
    let moved = records
        .iter()
        .filter(|(_, _, _, lottery, control)| lottery != control)
        .count();
    let (overall, _) = mean_var(&rows.iter().map(|(_, d, _)| *d).collect::<Vec<f64>>());
    println!(
        "{moved} of {games} draws moved the outcome at all; \
         mean score-share delta of a draw: {overall:+.4}\n"
    );

    let priced = contrasts(&rows, factors.len());
    let mut order: Vec<usize> = (0..factors.len()).collect();
    order.sort_by(|&a, &b| {
        let ta = priced[a].contrast / priced[a].se;
        let tb = priced[b].contrast / priced[b].se;
        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
    });
    println!(
        "{:<28} {:>5} {:>5} {:>9} {:>8} {:>6} {:>8}",
        "factor", "n_off", "n_on", "contrast", "se", "t", "win"
    );
    for index in order {
        let c = &priced[index];
        let t = c.contrast / c.se;
        println!(
            "{:<28} {:>5} {:>5} {:>+9.4} {:>8.4} {:>+6.1} {:>+8.3}",
            factors[index].1, c.n_off, c.n_on, c.contrast, c.se, t, c.win_contrast
        );
    }
    println!(
        "\ncontrast = mean(score-share delta | withheld) − mean(| kept): negative \
         means withholding hurt, so the treatment is an asset at this mixture. \
         |t| >= 2 is a screening signal, priced against agents missing a random \
         ~{:.0}% of the bundle — confirm on the single-flag arm at a disjoint \
         seed before deciding anything. A flat zero may be a factor that never \
         fired at this profile (fires-check before believing it).",
        density * 100.0
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_draw_is_deterministic_and_respects_density() {
        assert_eq!(draw(7, 3, 55, 0.5), draw(7, 3, 55, 0.5));
        assert_ne!(draw(7, 3, 55, 0.5), draw(7, 4, 55, 0.5));
        assert!(draw(7, 3, 55, 0.0).is_empty());
        assert_eq!(draw(7, 3, 55, 1.0).len(), 55);
        // Density is a rate, not a quota: over many draws the withheld share
        // must sit near it, or the design matrix is not the one advertised.
        let total: usize = (0..200).map(|game| draw(11, game, 55, 0.5).len()).sum();
        let share = total as f64 / (200.0 * 55.0);
        assert!((share - 0.5).abs() < 0.05, "withheld share {share}");
    }

    #[test]
    fn every_default_factor_resolves_to_a_registered_disabler() {
        let factors = default_factors();
        assert_eq!(
            factors.len(),
            ENGINE_REPAIR_TREATMENTS.len(),
            "every engine-repair tag must have a LIVE_TREATMENTS row"
        );
    }

    #[test]
    fn unknown_factor_names_are_refused() {
        let error = resolve_factors("come-ashore,no-such-treatment").unwrap_err();
        assert!(error.contains("no-such-treatment"));
        assert!(
            error.contains("come-ashore"),
            "the error must list known tags"
        );
        // Field name and kebab tag both resolve, as in `victory_eval`.
        assert_eq!(resolve_factors("come_ashore").unwrap().len(), 1);
        assert_eq!(resolve_factors("come-ashore").unwrap().len(), 1);
    }

    #[test]
    fn the_marginal_contrast_recovers_a_planted_effect() {
        // Plant main effects on a real design matrix: factor 0 costs 0.10 of
        // score share when withheld, factor 1 gains 0.05, factor 2 is null.
        // Deterministic noise from the same RNG family keeps the test exact.
        let rows: Vec<(Vec<usize>, f64, f64)> = (0..400)
            .map(|game| {
                let withheld = draw(99, game, 3, 0.5);
                let mut noise_rng = Rng::new(1_000_000 + game);
                let noise = (noise_rng.f64() - 0.5) * 0.02;
                let mut delta = noise;
                if withheld.contains(&0) {
                    delta -= 0.10;
                }
                if withheld.contains(&1) {
                    delta += 0.05;
                }
                (withheld, delta, 0.0)
            })
            .collect();
        let priced = contrasts(&rows, 3);
        assert!(
            (priced[0].contrast + 0.10).abs() < 0.01,
            "{}",
            priced[0].contrast
        );
        assert!(
            (priced[1].contrast - 0.05).abs() < 0.01,
            "{}",
            priced[1].contrast
        );
        assert!(priced[2].contrast.abs() < 0.01, "{}", priced[2].contrast);
        assert!(priced[0].contrast / priced[0].se < -2.0);
        assert!(priced[1].contrast / priced[1].se > 2.0);
        assert!((priced[2].contrast / priced[2].se).abs() < 2.0);
        for c in &priced {
            assert!(c.n_off + c.n_on == 400);
            assert!(c.n_off > 100, "density 0.5 must populate both halves");
        }
    }
}
