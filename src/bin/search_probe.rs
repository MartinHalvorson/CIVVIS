//! Screen a macro-search change in two minutes instead of forty.
//!
//! Every strength hypothesis in this repository has been decided by a paired
//! `ai_eval` run, and the evaluator needs about a hundred maps before it can
//! resolve anything — twenty-map runs on it have inverted twice, in opposite
//! directions. That is the right instrument for *deciding*, and a very
//! expensive one for *triaging*. Most hypotheses do not deserve it: several
//! have been discovered, after the run, to have changed nothing the search
//! could see.
//!
//! This measures the quantity that turned out to explain the one macro-search
//! change that has been promoted for its own sake. `StrategicAi` picks a lane
//! by projecting the adaptive baseline and each enabled lane and comparing the
//! resulting values, so **the spread between those values is the resolution of
//! the whole search**. Projecting each branch from the plan in force rather
//! than from a newly constructed planner roughly doubled it — 0.031 to 0.062
//! at four players — and won 87 mirrored map directions to 34 (`docs/EVAL.md`,
//! 2026-07-26).
//!
//! What the number is and is not:
//!
//! - **A change that does not move the spread cannot change a decision**, so a
//!   flat reading is a refutation and is worth having for two minutes. This is
//!   the generalisation of the fires-check every recent PR carries.
//! - **A change that moves it might still be worthless or harmful**, because
//!   noise separates branches as readily as signal does. A moved spread earns
//!   a pre-registered `ai_eval` run; it does not substitute for one.
//!
//! So: screen here, decide there. Never promote on this number.
//!
//! ```text
//! search_probe --players 4 --maps 24 --warmup 60 --cold
//! search_probe --players 4 --maps 24 --warmup 60 --horizon 80
//! ```
//!
//! The baseline is always a stock `StrategicAi`; the flags describe the
//! treatment. Both are measured **on the same positions with the same agent**,
//! so the comparison is paired and the sign test over positions is meaningful.
use civvis::ai::{Ai, AdvancedAi, VictoryTarget};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::strategic::{ReviewPath, StrategicAi};

/// What the rollouts would have answered at a position a prior answered
/// instead, and which prior it was.
struct PriorAudit {
    path: ReviewPath,
    prior: Option<VictoryTarget>,
    searched: Option<VictoryTarget>,
    spread: f64,
    decided: usize,
    branches: usize,
}

/// `choose_rollout_target`'s rule, reproduced from the values so the audit
/// does not need the private method.
fn searched_target(values: &[(f64, Option<VictoryTarget>)]) -> Option<VictoryTarget> {
    let adaptive = values
        .iter()
        .find_map(|(value, target)| target.is_none().then_some(*value))?;
    let mut best: Option<(f64, VictoryTarget)> = None;
    for (value, target) in values {
        let Some(target) = target else { continue };
        if best.is_none_or(|(top, _)| *value > top) {
            best = Some((*value, *target));
        }
    }
    best.filter(|(value, _)| *value > adaptive + 0.01)
        .map(|(_, target)| target)
}

fn path_name(path: ReviewPath) -> &'static str {
    match path {
        ReviewPath::DuelReligion => "duel-religion",
        ReviewPath::UrgentCounter => "urgent-counter",
        ReviewPath::IrreversibleReligion => "irreversible-religion",
        ReviewPath::Rollouts => "rollouts",
    }
}

fn lane_name(target: Option<VictoryTarget>) -> String {
    match target {
        None => "adaptive".to_string(),
        Some(target) => format!("{target:?}").to_lowercase(),
    }
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

/// One sampled review: the dispersion of the branch values each configuration
/// saw, and whether that configuration would have committed to a lane.
struct Reading {
    spread: f64,
    decided: usize,
    branches: usize,
    commits: bool,
}

fn read(values: &[(f64, Option<VictoryTarget>)]) -> Reading {
    let low = values.iter().map(|(v, _)| *v).fold(f64::INFINITY, f64::min);
    let high = values
        .iter()
        .map(|(v, _)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    let adaptive = values
        .iter()
        .find_map(|(value, target)| target.is_none().then_some(*value))
        .unwrap_or(0.0);
    let best_lane = values
        .iter()
        .filter(|(_, target)| target.is_some())
        .map(|(value, _)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    Reading {
        spread: high - low,
        // A branch that reaches a decided game returns exactly 1.0 or 0.0.
        // Score share never lands on either, so equality is the right test.
        decided: values
            .iter()
            .filter(|(value, _)| *value == 1.0 || *value == 0.0)
            .count(),
        branches: values.len(),
        // Mirrors `choose_rollout_target`'s rule without reaching into it.
        commits: best_lane.is_finite() && best_lane > adaptive + 0.01,
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[index]
}

/// Two-sided sign test, the same one `ai_eval` reports over map directions.
fn sign_p(up: usize, down: usize) -> f64 {
    let n = up + down;
    if n == 0 {
        return 1.0;
    }
    let k = up.min(down);
    let mut tail = 0.0f64;
    for i in 0..=k {
        let mut term = 0.0f64; // log C(n, i)
        for j in 0..i {
            term += ((n - j) as f64).ln() - ((j + 1) as f64).ln();
        }
        tail += (term - (n as f64) * std::f64::consts::LN_2).exp();
    }
    (2.0 * tail).min(1.0)
}

fn summarise(label: &str, readings: &[&Reading]) {
    let mut spreads: Vec<f64> = readings.iter().map(|r| r.spread).collect();
    spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = spreads.len().max(1);
    let over = readings.iter().filter(|r| r.spread > 0.01).count();
    let commits = readings.iter().filter(|r| r.commits).count();
    let decided: usize = readings.iter().map(|r| r.decided).sum();
    let branches: usize = readings.iter().map(|r| r.branches).sum();
    println!(
        "  {label:<24} {:>8.4} {:>8.4} {:>8.4} {:>9.0}% {:>10.0}% {:>10.0}%",
        percentile(&spreads, 0.5),
        percentile(&spreads, 0.9),
        spreads.last().copied().unwrap_or(0.0),
        100.0 * over as f64 / n as f64,
        100.0 * decided as f64 / branches.max(1) as f64,
        100.0 * commits as f64 / n as f64,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let warmup = number(&args, "--warmup", 60) as u32;
    let seed0 = number(&args, "--seed", 900) as u64;
    let jobs = number(&args, "--jobs", 8);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 200) as u32;

    // Audit mode: sample the positions a prior answers instead of the search,
    // and ask what the search would have said there.
    //
    // Half of all reviews never reach the rollouts -- `urgent_counter` alone
    // takes about a third -- and in a duel the religious prior takes all of
    // them. That is the single largest documented restriction on this search,
    // and nobody has checked what it costs. Widening or deepening a search
    // that answers one review in two is worth less than finding out whether
    // the other one should have been answered by it.
    if flag(&args, "--priors") {
        audit_priors(
            players, maps, warmup, seed0, jobs, width, height, turns,
        );
        return;
    }

    let cold = flag(&args, "--cold");
    let rotate = flag(&args, "--rotate");
    let horizon = number(&args, "--horizon", 0) as u32;

    let mut treatment = Vec::new();
    if cold {
        treatment.push("--cold".to_string());
    }
    if rotate {
        treatment.push("--rotate".to_string());
    }
    if horizon > 0 {
        treatment.push(format!("--horizon {horizon}"));
    }
    if treatment.is_empty() {
        eprintln!(
            "search_probe: no treatment flag given, so both arms are the stock agent \
             and every reading will be identical. Pass at least one of --cold, --rotate, \
             --horizon N."
        );
        std::process::exit(2);
    }

    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut agent = StrategicAi::with_weights(Default::default());
        let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    agent.take_turn(&mut game, pid);
                } else {
                    rivals[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() {
            return None;
        }
        // A position a prior would answer never reaches the branch values, so
        // measuring its dispersion would describe a search that does not run.
        if agent.review_detailed(&game, 0).1 != ReviewPath::Rollouts {
            return None;
        }

        let base = read(&agent.lane_values(&game, 0));
        if cold {
            agent.continue_from_plan = false;
        }
        if rotate {
            agent.rotate_lanes = true;
        }
        if horizon > 0 {
            agent.horizon = horizon;
        }
        let treated = read(&agent.lane_values(&game, 0));
        Some((base, treated))
    });

    let sampled: Vec<(Reading, Reading)> = results.into_iter().flatten().collect();
    if sampled.is_empty() {
        println!("no position reached the rollouts in {maps} maps — nothing to measure");
        std::process::exit(1);
    }

    println!(
        "search_probe: {} of {maps} positions reached the rollouts \
         ({players}p {width}x{height}, warmup {warmup}, seeds {seed0}..)",
        sampled.len()
    );
    println!("treatment: {}", treatment.join(" "));
    println!();
    println!(
        "  {:<24} {:>8} {:>8} {:>8} {:>10} {:>11} {:>11}",
        "branch-value spread", "median", "p90", "max", ">margin", "decided", "would commit"
    );
    summarise("baseline (stock)", &sampled.iter().map(|(b, _)| b).collect::<Vec<_>>());
    summarise("treatment", &sampled.iter().map(|(_, t)| t).collect::<Vec<_>>());

    // Spread is max - min over the projected branches, so it is only
    // comparable between arms that projected the SAME NUMBER of branches. A
    // treatment that shrinks the candidate set -- `--rotate` cuts seven lanes
    // to about two -- lowers the spread mechanically, on every position, with
    // no bearing on quality. This is the same confound that makes the
    // commitment margin a function of the candidate set (docs/EVAL.md,
    // 2026-07-26): a maximum over fewer draws is smaller for free.
    let base_branches: usize = sampled.iter().map(|(b, _)| b.branches).sum();
    let treated_branches: usize = sampled.iter().map(|(_, t)| t.branches).sum();
    let comparable = base_branches == treated_branches;
    if !comparable {
        println!(
            "\n⚠ branch counts differ ({} vs {} projected branches in total), so the \
             spread columns are NOT comparable -- a maximum over fewer branches is \
             smaller for free. Read the commitment flips instead.",
            base_branches, treated_branches
        );
    }

    let up = sampled.iter().filter(|(b, t)| t.spread > b.spread).count();
    let down = sampled.iter().filter(|(b, t)| t.spread < b.spread).count();
    let same = sampled.len() - up - down;
    println!();
    println!(
        "paired: treatment spread higher on {up}, lower on {down}, identical on {same}; \
         two-sided sign p={:.4}{}",
        sign_p(up, down),
        if comparable { "" } else { "  [NOT COMPARABLE]" }
    );

    // The load-bearing line. A treatment that leaves every branch value
    // untouched cannot change a decision, whatever it does to the win rate,
    // and that has happened often enough here to be worth naming.
    if same == sampled.len() {
        println!(
            "\nINERT — every branch value is identical to the digit. This treatment cannot \
             change a decision; an evaluation of it would measure nothing."
        );
        std::process::exit(3);
    }
    // The decision, not the dispersion, is what the search emits. A treatment
    // can leave the spread distribution untouched and still choose otherwise
    // in a quarter of positions -- which is what the promoted `--cold`
    // comparison does -- so this line is the one to read.
    let gained = sampled
        .iter()
        .filter(|(b, t)| !b.commits && t.commits)
        .count();
    let lost = sampled
        .iter()
        .filter(|(b, t)| b.commits && !t.commits)
        .count();
    println!(
        "commitment flips: {} of {} positions decide differently ({gained} toward a lane, \
         {lost} toward adaptive; two-sided sign p={:.4})",
        gained + lost,
        sampled.len(),
        sign_p(gained, lost)
    );
    println!(
        "\nScreening only. A moved spread earns a pre-registered ai_eval run; it is not \
         evidence of strength — noise separates branches as readily as signal does."
    );
}


fn audit_priors(
    players: usize,
    maps: usize,
    warmup: u32,
    seed0: u64,
    jobs: usize,
    width: i32,
    height: i32,
    turns: u32,
) {
    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut agent = StrategicAi::with_weights(Default::default());
        let mut rivals: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    agent.take_turn(&mut game, pid);
                } else {
                    rivals[pid].take_turn(&mut game, pid);
                }
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() {
            return None;
        }
        let (prior, path) = agent.review_detailed(&game, 0);
        if path == ReviewPath::Rollouts {
            return None;
        }
        // Force the projection the prior short-circuited.
        let values = agent.lane_values(&game, 0);
        let reading = read(&values);
        Some(PriorAudit {
            path,
            prior,
            searched: searched_target(&values),
            spread: reading.spread,
            decided: reading.decided,
            branches: reading.branches,
        })
    });

    let audits: Vec<PriorAudit> = results.into_iter().flatten().collect();
    if audits.is_empty() {
        println!("no position was answered by a prior in {maps} maps");
        std::process::exit(1);
    }

    println!(
        "prior audit: {} of {maps} positions were answered by a prior instead of the \
         rollouts ({players}p {width}x{height}, warmup {warmup}, seeds {seed0}..)",
        audits.len()
    );
    println!();
    println!(
        "  {:<24} {:>5} {:>10} {:>10} {:>10}",
        "prior", "n", "agrees", "median spread", "decided"
    );
    for path in [
        ReviewPath::DuelReligion,
        ReviewPath::UrgentCounter,
        ReviewPath::IrreversibleReligion,
    ] {
        let group: Vec<&PriorAudit> = audits.iter().filter(|a| a.path == path).collect();
        if group.is_empty() {
            continue;
        }
        let agrees = group.iter().filter(|a| a.prior == a.searched).count();
        let mut spreads: Vec<f64> = group.iter().map(|a| a.spread).collect();
        spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let decided: usize = group.iter().map(|a| a.decided).sum();
        let branches: usize = group.iter().map(|a| a.branches).sum();
        println!(
            "  {:<24} {:>5} {:>9.0}% {:>13.4} {:>9.0}%",
            path_name(path),
            group.len(),
            100.0 * agrees as f64 / group.len() as f64,
            percentile(&spreads, 0.5),
            100.0 * decided as f64 / branches.max(1) as f64,
        );
    }

    let disagreements: Vec<&PriorAudit> =
        audits.iter().filter(|a| a.prior != a.searched).collect();
    println!();
    println!(
        "the prior and the search disagree on {} of {} positions ({:.0}%)",
        disagreements.len(),
        audits.len(),
        100.0 * disagreements.len() as f64 / audits.len() as f64
    );
    let mut shown = 0;
    for audit in &disagreements {
        if shown >= 8 {
            break;
        }
        println!(
            "    {:<22} prior chose {:<11} search would choose {}",
            path_name(audit.path),
            lane_name(audit.prior),
            lane_name(audit.searched)
        );
        shown += 1;
    }

    println!(
        "\nAgreement is not correctness -- neither answer is known to be right here. \
         The disagreement rate is what a treatment that weakens a prior would act on, \
         and a rate near zero would say the prior is free and this line is closed."
    );
}
