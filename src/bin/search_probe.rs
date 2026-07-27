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
use civvis::ai::{run_game, Ai, AdvancedAi, VictoryTarget};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::production::ProductionSearchAi;
use civvis::ai::Weights;
use civvis::evolve::{fitness_observations, EvoCfg};
use civvis::strategic::{Doctrine, ReviewPath, StrategicAi};

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
    // Production mode: does the *production* search see anything?
    //
    // `ProductionSearchAi` is a recorded negative result -- 9 map directions
    // to 21 -- and its module note names the diagnosis it was never run
    // against: "if every branch returns the same number the horizon is too
    // short for the build to land, and no win rate would say so." It exposes
    // `candidate_values` for exactly that check. Nobody has taken it.
    // Outcome mode: does labelling a build by the GAME'S RESULT rank
    // candidates differently from score share?
    //
    // Two lines closed this week both ended at the same sentence: score share
    // is not win probability, and the lane search only works because its
    // branches sometimes reach a decided game and return exactly 1.0 or 0.0.
    // The proposed repair is an offline labeller that continues each candidate
    // to a real result. Before building one, measure whether the label it
    // would produce disagrees with the proxy it would replace. If it agrees,
    // the labeller is dead too and nobody spends a week on it.
    // Genome mode: does the 40-scalar tuning axis have headroom?
    //
    // `elo::builtin_ai` resolves every strategic agent through
    // `load_champion("evolved").unwrap_or_default()`, `evolved/` is
    // gitignored, and no `best.json` exists on this machine or in a fresh
    // clone -- so the shipped agents play `Weights::default()` and the 40
    // evolved weights have never been evolved. Before spending hours of GA on
    // that, ask the same question this tool asks of everything else: is the
    // effect bigger than the noise floor?
    //
    // The genomes are the four `Doctrine` perturbations, which are bounded by
    // evolution's own per-gene clamps -- so this measures the marginal value
    // of moving inside the space a GA would search, from a common position,
    // paired.
    // Selection mode: is the breeding statistic monotone in strength?
    //
    // #457 split ranking from promotion — breeding consumes
    // `50*P*score_share + 12*P*combat_share`, a continuous quantity with far
    // lower variance than a win indicator, while the SPRT still promotes on
    // wins alone. That is a sound variance-reduction argument *if* the
    // continuous statistic orders genomes the same way strength does.
    //
    // Nobody has checked. The obvious evidence — generation winners scoring
    // high on fitness and low on validation — is confounded by the winner's
    // curse, because "best of 24 noisy estimates" regresses on any later
    // measurement by construction. This avoids that entirely: every genome in
    // the population is measured, none is selected, and the win indicator and
    // the selection value come from *the same games*.
    if flag(&args, "--selection") {
        audit_selection(players, maps, seed0, jobs, width, height, turns);
        return;
    }

    if flag(&args, "--genome") {
        let replicas = number(&args, "--replicas", 5);
        audit_genome(
            players, maps, warmup, seed0, jobs, width, height, turns, replicas,
        );
        return;
    }

    if flag(&args, "--outcome") {
        let replicas = number(&args, "--replicas", 1);
        audit_outcome(
            players, maps, warmup, seed0, jobs, width, height, turns, replicas,
        );
        return;
    }

    if flag(&args, "--production") {
        audit_production(players, maps, warmup, seed0, jobs, width, height, turns);
        return;
    }

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
        // Keep the rollout-answered reviews too. The question the audit is
        // really asking is who decides this agent's lanes, and that needs
        // both populations counted the same way.
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

    let all: Vec<PriorAudit> = results.into_iter().flatten().collect();
    let searched_reviews: Vec<&PriorAudit> = all
        .iter()
        .filter(|a| a.path == ReviewPath::Rollouts)
        .collect();
    let audits: Vec<&PriorAudit> = all
        .iter()
        .filter(|a| a.path != ReviewPath::Rollouts)
        .collect();
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
        let group: Vec<&&PriorAudit> = audits.iter().filter(|a| a.path == path).collect();
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

    let disagreements: Vec<&&PriorAudit> =
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

    // Who actually decides this agent's lanes. A prior always names one; the
    // rollouts name one only when a lane clears the adaptive baseline by the
    // commitment margin, and most of the time none does. Counting both the
    // same way is the point of the audit.
    let prior_decisions = audits.iter().filter(|a| a.prior.is_some()).count();
    let search_decisions = searched_reviews
        .iter()
        .filter(|a| a.searched.is_some())
        .count();
    println!();
    println!(
        "who decides the lane, over {} sampled reviews:",
        all.len()
    );
    println!(
        "  priors    answered {:>4} reviews and named a lane in {:>4} ({:.0}%)",
        audits.len(),
        prior_decisions,
        100.0 * prior_decisions as f64 / audits.len().max(1) as f64
    );
    println!(
        "  rollouts  answered {:>4} reviews and named a lane in {:>4} ({:.0}%)",
        searched_reviews.len(),
        search_decisions,
        100.0 * search_decisions as f64 / searched_reviews.len().max(1) as f64
    );
    if search_decisions > 0 {
        println!(
            "  -> the priors make {:.1}x as many lane decisions as the search does",
            prior_decisions as f64 / search_decisions as f64
        );
    }

    println!(
        "\nAgreement is not correctness -- neither answer is known to be right here.\n\
         \n\
         ⚠ A disagreement rate is an upper bound on behavioural impact, and a loose \
         one. `adaptive` is not a lane: it hands the turn back to AdvancedAi's own \
         victory planner, which frequently picks the same lane the prior named. \
         Removing the irreversible-Prophet prior flipped 85% of its reviews from \
         religion to adaptive and moved religious commitment only 30.3% to 26.8%, \
         religious victories 171 to 164, and paired score not at all (49.6%, 240 maps, \
         p=0.8450). Read this column as `the label changed`, never as `the play \
         changed`."
    );
}


fn audit_production(
    players: usize,
    maps: usize,
    warmup: u32,
    seed0: u64,
    jobs: usize,
    width: i32,
    height: i32,
    turns: u32,
) {
    struct CityReading {
        candidates: usize,
        spread: f64,
        distinct: usize,
        agrees_with_deep: bool,
    }

    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() {
            return Vec::new();
        }
        let agent = ProductionSearchAi::new();
        let mut deep = ProductionSearchAi::new();
        deep.max_horizon = 200;
        let mut out = Vec::new();
        for cid in game.player_city_ids(0) {
            let values = agent.candidate_values(&game, 0, cid);
            if values.len() < 2 {
                continue;
            }
            // Does the CHOICE depend on the horizon? Separation says the
            // evaluator can tell the candidates apart; this says whether it
            // orders them the same way once every payoff has landed.
            let best = |vals: &[(civvis::game::Item, f64)]| -> Option<String> {
                vals.iter()
                    .fold(None, |top: Option<&(civvis::game::Item, f64)>, cand| {
                        match top.is_none_or(|t| cand.1 > t.1) {
                            true => Some(cand),
                            false => top,
                        }
                    })
                    .map(|(item, _)| format!("{item:?}"))
            };
            let shallow_pick = best(&values);
            let deep_pick = best(&deep.candidate_values(&game, 0, cid));
            let agrees = shallow_pick == deep_pick;
            let low = values.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
            let high = values
                .iter()
                .map(|(_, v)| *v)
                .fold(f64::NEG_INFINITY, f64::max);
            // How many candidates the evaluator can actually tell apart. If
            // this is 1 the search is choosing among identical numbers and is
            // deciding by enumeration order, not by projection.
            let mut seen: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
            seen.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            seen.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            out.push(CityReading {
                candidates: values.len(),
                spread: high - low,
                distinct: seen.len(),
                agrees_with_deep: agrees,
            });
        }
        out
    });

    let readings: Vec<CityReading> = results.into_iter().flatten().collect();
    if readings.is_empty() {
        println!("no city offered two or more candidate builds in {maps} maps");
        std::process::exit(1);
    }

    let mut spreads: Vec<f64> = readings.iter().map(|r| r.spread).collect();
    spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let blind = readings.iter().filter(|r| r.distinct <= 1).count();
    let candidates: usize = readings.iter().map(|r| r.candidates).sum();
    let distinct: usize = readings.iter().map(|r| r.distinct).sum();

    println!(
        "production audit: {} city decisions over {maps} maps \
         ({players}p {width}x{height}, warmup {warmup}, seeds {seed0}..)",
        readings.len()
    );
    println!();
    println!("  candidates projected per decision   {:.1}", candidates as f64 / readings.len() as f64);
    println!("  values the evaluator can separate   {:.1}", distinct as f64 / readings.len() as f64);
    println!("  median spread across candidates     {:.6}", percentile(&spreads, 0.5));
    println!("  p90 spread                          {:.6}", percentile(&spreads, 0.9));
    println!("  max spread                          {:.6}", spreads.last().copied().unwrap_or(0.0));
    println!(
        "  decisions where every candidate scores the SAME   {blind} of {} ({:.0}%)",
        readings.len(),
        100.0 * blind as f64 / readings.len() as f64
    );
    let agrees = readings.iter().filter(|r| r.agrees_with_deep).count();
    println!(
        "  same pick at horizon 200 as at the shipped ceiling  {agrees} of {} ({:.0}%)",
        readings.len(),
        100.0 * agrees as f64 / readings.len() as f64
    );
    println!(
        "\nA decision whose candidates all score alike is decided by enumeration order, \
         not by projection. That is the failure `PolicyAi` had on 96% of its candidates, \
         and the one this module's own note predicts for a horizon shorter than the \
         build's payoff."
    );
}


/// Distinct-but-sane opponent policies, so a candidate can be continued
/// against more than one future.
///
/// The engine is deterministic: the same position played by the same agents
/// always produces the same game, so a single continuation cannot be denoised
/// by repeating it. Varying the *opponents* is the only replication this
/// design admits. These are the four doctrine perturbations of the stock
/// genome — bounded by evolution's own per-gene clamps, so each is a play
/// style rather than a broken agent — plus the frozen legacy planner.
fn opponent_pool(index: usize) -> Box<dyn Ai> {
    let base = Weights::default();
    match index % 5 {
        0 => Box::new(AdvancedAi::new()) as Box<dyn Ai>,
        4 => Box::new(AdvancedAi::legacy()) as Box<dyn Ai>,
        other => Box::new(AdvancedAi::with_weights(
            Doctrine::ALL[other].apply(&base),
        )) as Box<dyn Ai>,
    }
}

fn audit_outcome(
    players: usize,
    maps: usize,
    warmup: u32,
    seed0: u64,
    jobs: usize,
    width: i32,
    height: i32,
    turns: u32,
    replicas: usize,
) {
    struct Decision {
        candidates: usize,
        /// Candidates whose continuation this seat won.
        wins: usize,
        /// The proxy's pick also won its continuation.
        proxy_pick_won: bool,
        /// Proxy pick and outcome pick are the same item.
        agrees: bool,
        /// Outcomes are not all identical, so the label says something.
        discriminates: bool,
        /// Candidates whose replicas did not all agree — the direct evidence
        /// that a single continuation is noise.
        mixed_candidates: usize,
        /// Highest candidate win rate minus lowest, at this decision.
        rate_spread: f64,
    }

    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() {
            return Vec::new();
        }
        let agent = ProductionSearchAi::new();
        let mut out = Vec::new();
        // One city per map: a full continuation per candidate is about
        // seventy times the cost of a game, and the question is the
        // disagreement rate, not coverage.
        for cid in game.player_city_ids(0).into_iter().take(1) {
            let scored = agent.candidate_values(&game, 0, cid);
            if scored.len() < 2 {
                continue;
            }
            let mut labelled: Vec<(String, f64, bool)> = Vec::new();
            let mut rates: Vec<f64> = Vec::new();
            let mut mixed = 0usize;
            for (item, proxy) in &scored {
                let mut wins = 0usize;
                let mut runs = 0usize;
                for replica in 0..replicas.max(1) {
                    let mut sim = game.clone();
                    if sim
                        .apply(
                            0,
                            &Action::Produce {
                                city: cid,
                                item: item.clone(),
                            },
                        )
                        .is_err()
                    {
                        continue;
                    }
                    // The searching seat keeps the stock agent in every
                    // replica; only the opponents vary, which is what makes
                    // the win rate a property of the candidate.
                    let mut ais: Vec<Box<dyn Ai>> = sim
                        .players
                        .iter()
                        .map(|p| {
                            if p.id == 0 {
                                Box::new(AdvancedAi::new()) as Box<dyn Ai>
                            } else {
                                opponent_pool(replica + p.id)
                            }
                        })
                        .collect();
                    run_game(&mut sim, &mut ais);
                    runs += 1;
                    if sim.winner == Some(0) {
                        wins += 1;
                    }
                }
                if runs == 0 {
                    continue;
                }
                if wins > 0 && wins < runs {
                    mixed += 1;
                }
                rates.push(wins as f64 / runs as f64);
                labelled.push((format!("{item:?}"), *proxy, wins * 2 > runs));
            }
            if labelled.len() < 2 {
                continue;
            }
            let wins = labelled.iter().filter(|(_, _, won)| *won).count();
            let proxy_pick = labelled
                .iter()
                .fold(None, |top: Option<&(String, f64, bool)>, cand| {
                    match top.is_none_or(|t| cand.1 > t.1) {
                        true => Some(cand),
                        false => top,
                    }
                })
                .expect("labelled is non-empty");
            // The outcome label's pick: any winning candidate. Ties keep
            // enumeration order, as everywhere else in this codebase.
            let outcome_pick = labelled.iter().find(|(_, _, won)| *won);
            let low = rates.iter().copied().fold(f64::INFINITY, f64::min);
            let high = rates.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            out.push(Decision {
                candidates: labelled.len(),
                wins,
                proxy_pick_won: proxy_pick.2,
                agrees: outcome_pick.is_none_or(|(name, _, _)| *name == proxy_pick.0),
                discriminates: wins > 0 && wins < labelled.len(),
                mixed_candidates: mixed,
                rate_spread: high - low,
            });
        }
        out
    });

    let decisions: Vec<Decision> = results.into_iter().flatten().collect();
    if decisions.is_empty() {
        println!("no city decision could be continued to a result in {maps} maps");
        std::process::exit(1);
    }
    let n = decisions.len();
    let discriminating: Vec<&Decision> = decisions.iter().filter(|d| d.discriminates).collect();
    let candidates: usize = decisions.iter().map(|d| d.candidates).sum();

    println!(
        "outcome audit: {n} city decisions, every candidate continued to a real result \
         ({players}p {width}x{height}, warmup {warmup}, {turns}-turn budget, seeds {seed0}..)"
    );
    println!();
    println!("  candidates continued per decision              {:.1}", candidates as f64 / n as f64);
    println!(
        "  decisions where the label DISCRIMINATES         {} of {n} ({:.0}%)",
        discriminating.len(),
        100.0 * discriminating.len() as f64 / n as f64
    );
    if !discriminating.is_empty() {
        let agrees = discriminating.iter().filter(|d| d.agrees).count();
        let proxy_won = discriminating.iter().filter(|d| d.proxy_pick_won).count();
        println!(
            "  ...of those, proxy pick == outcome pick        {agrees} of {} ({:.0}%)",
            discriminating.len(),
            100.0 * agrees as f64 / discriminating.len() as f64
        );
        println!(
            "  ...of those, the proxy's pick WON its game     {proxy_won} of {} ({:.0}%)",
            discriminating.len(),
            100.0 * proxy_won as f64 / discriminating.len() as f64
        );
    }
    if replicas > 1 {
        let mixed: usize = decisions.iter().map(|d| d.mixed_candidates).sum();
        let mut spreads: Vec<f64> = decisions.iter().map(|d| d.rate_spread).collect();
        spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let separating = decisions.iter().filter(|d| d.rate_spread >= 0.4).count();
        println!();
        println!("  replicas per candidate                        {replicas}");
        println!(
            "  candidates whose replicas DISAGREED           {mixed} of {candidates} ({:.0}%)",
            100.0 * mixed as f64 / candidates.max(1) as f64
        );
        println!(
            "  median win-rate spread across candidates      {:.2}",
            percentile(&spreads, 0.5)
        );
        println!(
            "  decisions separating by >= 0.4 win rate       {separating} of {n} ({:.0}%)",
            100.0 * separating as f64 / n as f64
        );
    }

    if replicas > 1 {
        // The criterion this mode exists to evaluate. A win rate over K
        // replicas carries a standard error of sqrt(p(1-p)/K), about 0.224 at
        // K=5. If the spread BETWEEN candidates is smaller than the error on
        // each one, no amount of corpus-building separates them: the label is
        // under its own noise floor and a search over this decision is
        // measuring nothing it can act on.
        let se = (0.25f64 / replicas as f64).sqrt();
        let mut spreads: Vec<f64> = decisions.iter().map(|d| d.rate_spread).collect();
        spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = percentile(&spreads, 0.5);
        println!();
        println!(
            "  per-candidate standard error at {replicas} replicas   {se:.3}   \
             vs median between-candidate spread {median:.3}"
        );
        if median < se {
            let needed = (0.25 / (median / 4.0).powi(2)).ceil() as usize;
            println!(
                "  -> UNDER THE NOISE FLOOR. Resolving a gap of {median:.2} at 4 sigma needs \
                 about {needed} replicas per candidate, {}x this run.",
                needed / replicas.max(1)
            );
        }
    }

    println!(
        "\nA decision where every continuation ends the same way carries no signal for a \
         labeller, whatever it costs to produce. Read the discrimination rate first: it \
         bounds how much an outcome-labelled corpus could ever teach about this decision, \
         and the noise-floor line second: it says what that would cost."
    );
}


fn audit_genome(
    players: usize,
    maps: usize,
    warmup: u32,
    seed0: u64,
    jobs: usize,
    width: i32,
    height: i32,
    turns: u32,
    replicas: usize,
) {
    let results = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet(&game);
        for _ in 0..warmup {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        if game.winner.is_some() {
            return None;
        }
        let base = Weights::default();
        let mut rates = Vec::new();
        for doctrine in Doctrine::ALL {
            let genome = doctrine.apply(&base);
            let mut wins = 0usize;
            for replica in 0..replicas.max(1) {
                let mut sim = game.clone();
                let mut ais: Vec<Box<dyn Ai>> = sim
                    .players
                    .iter()
                    .map(|p| {
                        if p.id == 0 {
                            Box::new(AdvancedAi::with_weights(genome.clone())) as Box<dyn Ai>
                        } else {
                            opponent_pool(replica + p.id)
                        }
                    })
                    .collect();
                run_game(&mut sim, &mut ais);
                if sim.winner == Some(0) {
                    wins += 1;
                }
            }
            rates.push((doctrine.name(), wins as f64 / replicas.max(1) as f64));
        }
        Some(rates)
    });

    let sampled: Vec<Vec<(&str, f64)>> = results.into_iter().flatten().collect();
    if sampled.is_empty() {
        println!("no ordinary mid-game position was reached in {maps} maps");
        std::process::exit(1);
    }
    let n = sampled.len();
    let mut spreads: Vec<f64> = sampled
        .iter()
        .map(|rates| {
            let low = rates.iter().map(|(_, r)| *r).fold(f64::INFINITY, f64::min);
            let high = rates
                .iter()
                .map(|(_, r)| *r)
                .fold(f64::NEG_INFINITY, f64::max);
            high - low
        })
        .collect();
    spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let se = (0.25f64 / replicas.max(1) as f64).sqrt();
    let median = percentile(&spreads, 0.5);

    println!(
        "genome audit: {n} positions, {} bounded genomes x {replicas} opponent replicas \
         ({players}p {width}x{height}, warmup {warmup}, {turns}-turn budget, seeds {seed0}..)",
        Doctrine::ALL.len()
    );
    println!();
    for doctrine in Doctrine::ALL {
        let mean: f64 = sampled
            .iter()
            .filter_map(|rates| {
                rates
                    .iter()
                    .find(|(name, _)| *name == doctrine.name())
                    .map(|(_, rate)| *rate)
            })
            .sum::<f64>()
            / n as f64;
        println!("  {:<14} mean win rate {mean:.3}", doctrine.name());
    }
    println!();
    println!("  median spread across genomes at a position   {median:.3}");
    println!("  p90 spread                                   {:.3}", percentile(&spreads, 0.9));
    println!("  per-candidate standard error                 {se:.3}");
    if median > se {
        println!(
            "  -> ABOVE the noise floor: bounded genome moves shift the win rate by more \
             than a {replicas}-replica measurement's error, so this axis has something to find."
        );
    } else {
        let needed = (0.25 / (median.max(1e-6) / 4.0).powi(2)).ceil() as usize;
        println!(
            "  -> UNDER the noise floor: resolving a {median:.2} gap at 4 sigma needs about \
             {needed} replicas per genome."
        );
    }
}


fn audit_selection(
    players: usize,
    population: usize,
    seed0: u64,
    jobs: usize,
    width: i32,
    height: i32,
    turns: u32,
) {
    // A stand-in for `evolve::mutate`, which is crate-private: perturb each
    // gene and clamp to the same per-gene bounds evolution respects, so every
    // member is a play style rather than a broken agent.
    fn perturbed(index: usize) -> Weights {
        let base = Weights::default();
        if index == 0 {
            return base;
        }
        let mut state = 0x9E3779B97F4A7C15u64 ^ (index as u64).wrapping_mul(0x2545F4914F6CDD1D);
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut genes = base.to_vec();
        for (gene, (low, high)) in genes.iter_mut().zip(Weights::bounds()) {
            // ±25% on a third of the genes, the same scale `Doctrine` uses.
            if next() < 0.34 {
                *gene *= 0.75 + 0.5 * next();
            }
            *gene = gene.clamp(low, high);
        }
        Weights::from_vec(&genes)
    }

    let games = number(&std::env::args().collect::<Vec<_>>(), "--games", 48);
    let cfg = EvoCfg {
        generations: 1,
        pop: population,
        games,
        players,
        width,
        height,
        max_turns: turns,
        seed: seed0,
        threads: jobs,
        dir: "evolved".to_string(),
    };
    let opponents = vec![Weights::default()];

    let results = parallel::map(population, jobs, move |index| {
        let genome = perturbed(index);
        // Common random numbers: `fitness_observations` derives its seeds from
        // (cfg.seed, gen, game index) only, so every genome sees the same maps,
        // seats and turn budgets. The comparison across the population is paired.
        let obs = fitness_observations(&genome, &opponents, &cfg, 0, games);
        if obs.is_empty() {
            return None;
        }
        let selection: f64 = obs
            .iter()
            .map(|o| o.selection_value(players))
            .sum::<f64>()
            / obs.len() as f64;
        let wins = obs.iter().filter(|o| o.won).count() as f64 / obs.len() as f64;
        Some((selection, wins))
    });

    let pairs: Vec<(f64, f64)> = results.into_iter().flatten().collect();
    if pairs.len() < 3 {
        println!("population too small to correlate");
        std::process::exit(1);
    }
    let n = pairs.len() as f64;
    let mean = |f: &dyn Fn(&(f64, f64)) -> f64| pairs.iter().map(f).sum::<f64>() / n;
    let (ms, mw) = (mean(&|p| p.0), mean(&|p| p.1));
    let cov: f64 = pairs.iter().map(|p| (p.0 - ms) * (p.1 - mw)).sum::<f64>() / n;
    let sds = (pairs.iter().map(|p| (p.0 - ms).powi(2)).sum::<f64>() / n).sqrt();
    let sdw = (pairs.iter().map(|p| (p.1 - mw).powi(2)).sum::<f64>() / n).sqrt();
    let r = if sds > 0.0 && sdw > 0.0 {
        cov / (sds * sdw)
    } else {
        0.0
    };

    let argmax = |f: &dyn Fn(&(f64, f64)) -> f64| {
        pairs
            .iter()
            .enumerate()
            .fold(None, |top: Option<(usize, f64)>, (i, p)| {
                let v = f(p);
                match top.is_none_or(|(_, t)| v > t) {
                    true => Some((i, v)),
                    false => top,
                }
            })
            .expect("non-empty")
    };
    let (best_selection, _) = argmax(&|p| p.0);
    let (best_wins, _) = argmax(&|p| p.1);

    println!(
        "selection audit: {} genomes x {games} common-seed games \
         ({players}p {width}x{height}, {turns}-turn budget, seed {seed0})",
        pairs.len()
    );
    println!();
    println!("  mean selection value   {ms:.2}   (sd {sds:.2})");
    println!("  mean win rate          {mw:.3}   (sd {sdw:.3})");
    println!();
    println!("  correlation(selection value, win rate) = {r:+.3}");
    println!(
        "  breeding would pick genome #{best_selection}; the strongest is #{best_wins} \
         (win rate {:.3} vs {:.3})",
        pairs[best_selection].1, pairs[best_wins].1
    );
    // A correlation is only as good as the reliability of each side. The win
    // rate is a Bernoulli mean over `games`, so it carries a standard error of
    // sqrt(p(1-p)/games). If that error is not comfortably below the observed
    // spread ACROSS genomes, the win-rate column is mostly measurement noise,
    // every correlation with it is attenuated toward zero, and no verdict can
    // be read off it. Saying so is the whole point of computing it.
    let win_se = (mw * (1.0 - mw) / games as f64).sqrt();
    let reliable = sdw > 2.0 * win_se;
    println!(
        "  win-rate SE per genome {win_se:.3} vs observed spread {sdw:.3} across genomes"
    );
    println!();
    if !reliable {
        let signal = (sdw * sdw - win_se * win_se).max(0.0).sqrt();
        let needed = if signal > 0.0 {
            (mw * (1.0 - mw) / (signal / 2.0).powi(2)).ceil() as usize
        } else {
            0
        };
        println!(
            "  -> NO VERDICT. The spread across genomes is not above the error on each \
             one, so the win-rate column is consistent with pure noise and the correlation \
             above is attenuated from an unknown value."
        );
        if needed > 0 {
            println!("     About {needed} games/genome would make it readable.");
        } else {
            println!(
                "     The observed spread is entirely within measurement error, so the \
                 games/genome needed cannot be estimated from this run — raise it and \
                 re-measure."
            );
        }
        println!(
            "\nNo genome is selected here and both numbers come from the same games, so \
             this carries no winner's curse — but an unbiased estimate of nothing is \
             still nothing."
        );
        return;
    }
    if r > 0.5 {
        println!(
            "  -> MONOTONE ENOUGH: the continuous statistic orders genomes broadly the way \
             strength does, so ranking on it buys variance reduction without changing the \
             answer."
        );
    } else {
        println!(
            "  -> NOT MONOTONE: the statistic breeding consumes is only weakly related to \
             the statistic promotion measures, so the GA climbs one hill while the gate \
             guards another."
        );
    }
    println!(
        "\nNo genome is selected here and both numbers come from the same games, so this \
         carries no winner's curse — unlike a comparison of generation champions."
    );
}
