//! Where do Diplomatic Victory Points come from, and can anybody stop them?
//!
//! [`docs/COUNTERING_LEADERS.md`] closed the war-shaped counter: seven
//! treatments, three map profiles, every one null, because the response is
//! paid for in development and buys no wins. The World Congress is the one
//! counter in the game that is *not* paid for in development — a losing vote
//! is refunded in full ([`Game::resolve_congress`]) — and nothing has ever
//! measured it.
//!
//! Three things award a Diplomatic Victory Point, and only one of them is the
//! one people talk about:
//!
//! - **the exact prediction**: every voter who backs the winning outcome *and*
//!   the winning target takes +1, on *every* resolution, diplomatic or not;
//! - **`world_leader`**, on the ballot in every session from the Modern era:
//!   outcome A gives its target +2, outcome B takes 2 away;
//! - **wonders, techs and civics** carrying `diplomatic_victory_points`.
//!
//! A counter can only ever be as large as the source it denies, so this census
//! decomposes the twenty points a diplomatic victory needs into those three
//! before anybody builds a response to one of them. It also reads the vote
//! itself, because `AdvancedAi` weights its ballot by *its own* grand strategy
//! (three votes on the Diplomacy plan with 30 Favor, otherwise one) and never
//! by what is at stake — so a runaway diplomat outvotes each of its rivals
//! three to one at the exact moment they most need to outvote it.
//!
//! The readings, all per `world_leader` resolution:
//!
//! - **A-votes against B-votes** on the empire in the diplomatic lead, and how
//!   many rivals turned up at all;
//! - **who could have paid**: rivals holding the 10 and 30 Favor that a second
//!   and third vote cost, since a counter nobody can afford is not a counter;
//! - **the counterfactual flip**: how many sessions that ended +2 for the
//!   leader would have ended −2 had its rivals spent what they were holding.
//!
//! ```text
//! congress_census --players 6 --maps 24 --width 74 --height 46 \
//!     --city-states 9 --turns 400 --seed 980000
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, CongressSession, Game};
use civvis::parallel;
use std::collections::BTreeMap;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Favor a second and a third vote cost, from `Game::congress_vote_cost`
/// (`5·p·(p+1)` for `p` paid votes). Duplicated rather than exported: the
/// census must not be able to drift the engine's own schedule.
const SECOND_VOTE: f64 = 10.0;
const THIRD_VOTE: f64 = 30.0;

/// Split an `A:target` ballot the way the engine does. Pre-variety saves used
/// the bare target and are read as outcome A.
fn choice_parts(choice: &str) -> (&str, &str) {
    choice.split_once(':').unwrap_or(("A", choice))
}

/// Replay the engine's tally: the outcome with the most votes, then the target
/// with the most votes *within* that outcome. The engine breaks a tie on the
/// largest share of a voter's available Favor and then on the outcome letter;
/// this reports ties instead of guessing at them, so a flip is never claimed
/// on a coin the census cannot see.
fn tally(ballots: &BTreeMap<usize, (String, u32)>) -> Option<(String, String, bool)> {
    let mut outcome_totals: BTreeMap<String, u32> = BTreeMap::new();
    for (choice, votes) in ballots.values() {
        let (outcome, _) = choice_parts(choice);
        *outcome_totals.entry(outcome.to_string()).or_insert(0) += *votes;
    }
    let best_outcome = outcome_totals.values().copied().max()?;
    let mut leaders: Vec<String> = outcome_totals
        .iter()
        .filter(|(_, votes)| **votes == best_outcome)
        .map(|(outcome, _)| outcome.clone())
        .collect();
    leaders.sort();
    let outcome_tied = leaders.len() > 1;
    // The engine's last resort prefers the earlier letter, so A carries a tie
    // it cannot otherwise break.
    let winning_outcome = leaders.first()?.clone();

    let mut target_totals: BTreeMap<String, u32> = BTreeMap::new();
    for (choice, votes) in ballots.values() {
        let (outcome, target) = choice_parts(choice);
        if outcome == winning_outcome {
            *target_totals.entry(target.to_string()).or_insert(0) += *votes;
        }
    }
    let best_target = target_totals.values().copied().max()?;
    let mut target_leaders: Vec<String> = target_totals
        .iter()
        .filter(|(_, votes)| **votes == best_target)
        .map(|(target, _)| target.clone())
        .collect();
    target_leaders.sort();
    let tied = outcome_tied || target_leaders.len() > 1;
    Some((winning_outcome, target_leaders.first()?.clone(), tied))
}

#[derive(Default)]
struct Counts {
    /// Points the census can attribute, by source.
    from_prediction: i64,
    from_world_leader: i64,
    denied_by_world_leader: i64,
    /// Everything else that moved `dvp` — wonders, techs, civics.
    residual: i64,

    sessions: u64,
    resolutions: u64,
    /// Special Sessions, counted apart: they award nothing.
    emergencies: u64,
    world_leader_votes: u64,
    /// `world_leader` sessions by what happened to the diplomatic leader.
    wl_leader_gained: u64,
    wl_leader_denied: u64,
    wl_other_target: u64,
    wl_tied: u64,

    /// Ballots cast on `world_leader`, by side and by weight.
    a_on_leader_votes: u64,
    b_on_leader_votes: u64,
    rivals_voting_b: u64,
    rivals_abstaining: u64,
    rivals_voting_a: u64,
    /// Rivals who held enough Favor for a second or third vote when the
    /// session opened, and how many actually bought one.
    rivals_could_pay_second: u64,
    rivals_could_pay_third: u64,
    rivals_bought_extra: u64,

    /// Sessions where the leader gained and a fully-paid opposition would have
    /// denied instead, split by what the opposition could actually afford.
    flip_if_paid: u64,
    flip_if_free: u64,

    /// Fires-check for the treatment arms. A ballot is "aimed" when it names
    /// the empire that voter's own denial layer names; a vote is "bought" when
    /// a ballot carries more than one.
    ballots_cast: u64,
    ballots_aimed: u64,
    votes_bought: u64,
    /// Resolutions carrying a targeted penalty that actually passed, and how
    /// many landed on the empire that went on to win the game.
    penalties_passed: u64,
}

impl Counts {
    fn merge(&mut self, other: &Counts) {
        self.from_prediction += other.from_prediction;
        self.from_world_leader += other.from_world_leader;
        self.denied_by_world_leader += other.denied_by_world_leader;
        self.residual += other.residual;
        self.sessions += other.sessions;
        self.resolutions += other.resolutions;
        self.emergencies += other.emergencies;
        self.world_leader_votes += other.world_leader_votes;
        self.wl_leader_gained += other.wl_leader_gained;
        self.wl_leader_denied += other.wl_leader_denied;
        self.wl_other_target += other.wl_other_target;
        self.wl_tied += other.wl_tied;
        self.a_on_leader_votes += other.a_on_leader_votes;
        self.b_on_leader_votes += other.b_on_leader_votes;
        self.rivals_voting_b += other.rivals_voting_b;
        self.rivals_abstaining += other.rivals_abstaining;
        self.rivals_voting_a += other.rivals_voting_a;
        self.rivals_could_pay_second += other.rivals_could_pay_second;
        self.rivals_could_pay_third += other.rivals_could_pay_third;
        self.rivals_bought_extra += other.rivals_bought_extra;
        self.flip_if_paid += other.flip_if_paid;
        self.flip_if_free += other.flip_if_free;
        self.ballots_cast += other.ballots_cast;
        self.ballots_aimed += other.ballots_aimed;
        self.votes_bought += other.votes_bought;
        self.penalties_passed += other.penalties_passed;
    }
}

struct MapReading {
    winner: Option<usize>,
    victory_type: String,
    end_turn: u32,
    peak_era: usize,
    /// Highest `dvp` any living major ever held, and the final table.
    peak_dvp: i64,
    final_dvp: Vec<i64>,
    /// One row per congress session: who held the diplomatic lead, who held
    /// the score lead, and who `rival_pressure` put closest to a win. The
    /// first of those is the only one `congress_choice` can aim at.
    aim: Vec<(usize, usize, usize)>,
    /// Every empire a targeted penalty actually landed on, so the arms can be
    /// compared on where the Congress's teeth ended up.
    penalised: Vec<usize>,
    counts: Counts,
}

/// The three resolutions whose outcome B costs its target real yields: a total
/// trade embargo, -20% growth, and no tile annexation from border growth.
const PENALTIES: [&str; 3] = ["trade_policy", "migration_treaty", "border_control_treaty"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 24);
    let width = number(&args, "--width", 24) as i32;
    let height = number(&args, "--height", 16) as i32;
    let turns = number(&args, "--turns", 400) as u32;
    let seed0 = number(&args, "--seed", 980_000) as u64;
    let city_states = number(&args, "--city-states", 0);
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    // Which congress behaviour the whole table plays. The census is otherwise
    // identical, so `--arm` reads what a treatment does to behaviour before any
    // question of what it does to strength.
    let arm = args
        .iter()
        .position(|a| a == "--arm")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or("ship")
        .to_string();
    if !matches!(arm.as_str(), "ship" | "counter" | "votes" | "hard") {
        eprintln!("--arm must be ship, counter, votes or hard");
        std::process::exit(2);
    }

    println!(
        "congress_census: {maps} maps, {players}p {width}x{height}, {city_states} city-states, \
         {turns} turns, seed {seed0}, arm {arm}"
    );

    let arm_label = arm.clone();
    let readings = parallel::map(maps, jobs, move |index| {
        let arm = arm_label.as_str();
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, city_states);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        for planner in fleet.iter_mut() {
            planner.congress_counter_leader = arm == "counter" || arm == "hard";
            planner.congress_counter_votes = arm == "votes" || arm == "hard";
        }
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();

        let mut counts = Counts::default();
        let mut peak_dvp = 0_i64;
        let mut peak_era = 0_usize;
        let mut end_turn = turns;
        // The session still being voted on, the Favor its members held when it
        // opened, and the DVP table from the turn before it resolved.
        let mut pending: Option<CongressSession> = None;
        let mut opening_favor: Vec<f64> = vec![0.0; game.players.len()];
        let mut dvp_prev: Vec<i64> = game.players.iter().map(|p| p.dvp).collect();
        let mut aim: Vec<(usize, usize, usize)> = Vec::new();
        let mut penalised: Vec<usize> = Vec::new();
        // `rival_pressure` reads nothing from the planner it is asked of, so
        // one probe answers for every seat.
        let probe = AdvancedAi::new();

        for turn in 0..turns {
            if game.winner.is_some() {
                end_turn = turn;
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
            end_turn = turn;
            peak_era = peak_era.max(game.world_era);
            for pid in majors.iter().copied() {
                peak_dvp = peak_dvp.max(game.players[pid].dvp);
            }

            // A session that is no longer the one being voted on has resolved,
            // whether the seat emptied or an Emergency took it.
            let observed = game.congress.clone();
            let resolved = match (&pending, &observed) {
                (Some(prior), Some(current)) if prior.convened != current.convened => {
                    Some(prior.clone())
                }
                (Some(prior), None) => Some(prior.clone()),
                _ => None,
            };

            if let Some(session) = resolved {
                counts.sessions += 1;
                // Attribute this turn's DVP movement to the vote where it can
                // be, and to everything else where it cannot.
                let mut attributed: BTreeMap<usize, i64> = BTreeMap::new();
                for resolution in &session.resolutions {
                    // A Special Session resolves down `resolve_emergency_session`,
                    // which convenes a coalition and refunds the losing side but
                    // awards no Diplomatic Victory Point at all. Attributing the
                    // stock +1 to its voters is how this census first read a
                    // negative residual: it was crediting points the engine
                    // never paid.
                    if resolution.id.starts_with("emergency:") {
                        counts.emergencies += 1;
                        continue;
                    }
                    counts.resolutions += 1;
                    let Some((outcome, target, tied)) = tally(&resolution.ballots) else {
                        continue;
                    };
                    for (voter, (choice, votes)) in &resolution.ballots {
                        let (cast_outcome, cast_target) = choice_parts(choice);
                        if cast_outcome == outcome && cast_target == target {
                            *attributed.entry(*voter).or_insert(0) += 1;
                            counts.from_prediction += 1;
                        }
                        // Fires-check: did this ballot name the empire this
                        // voter's own denial layer names, and did it pay for
                        // the naming?
                        counts.ballots_cast += 1;
                        if *votes > 1 {
                            counts.votes_bought += 1;
                        }
                        if fleet[*voter]
                            .denial_target(&game, *voter)
                            .is_some_and(|(rival, _)| cast_target == rival.to_string())
                        {
                            counts.ballots_aimed += 1;
                        }
                    }
                    // Where the Congress's teeth landed. Outcome B on any of
                    // these three costs its target real yields.
                    if PENALTIES.contains(&resolution.id.as_str()) && outcome == "B" {
                        if let Ok(hit) = target.parse::<usize>() {
                            counts.penalties_passed += 1;
                            penalised.push(hit);
                        }
                    }
                    if resolution.id != "world_leader" {
                        continue;
                    }
                    if tied {
                        counts.wl_tied += 1;
                    }

                    // Who the vote was actually about: the empire holding the
                    // diplomatic lead when the session opened, which is the
                    // empire `congress_choice` scores 900 to oppose.
                    let leader = majors
                        .iter()
                        .copied()
                        .filter(|pid| game.players[*pid].alive)
                        .max_by_key(|pid| (dvp_prev[*pid], std::cmp::Reverse(*pid)));
                    let Some(leader) = leader else { continue };

                    if let Ok(hit) = target.parse::<usize>() {
                        if outcome == "A" {
                            *attributed.entry(hit).or_insert(0) += 2;
                            counts.from_world_leader += 2;
                        } else {
                            let taken = dvp_prev[hit].min(2);
                            *attributed.entry(hit).or_insert(0) -= taken;
                            counts.denied_by_world_leader += taken;
                        }
                        if hit != leader {
                            counts.wl_other_target += 1;
                        } else if outcome == "A" {
                            counts.wl_leader_gained += 1;
                        } else {
                            counts.wl_leader_denied += 1;
                        }
                    }

                    // The ballot as cast, from the point of view of everybody
                    // who is not the leader.
                    let leader_key = leader.to_string();
                    let mut a_votes = 0_u32;
                    let mut b_votes = 0_u32;
                    for (voter, (choice, votes)) in &resolution.ballots {
                        counts.world_leader_votes += 1;
                        let (cast_outcome, cast_target) = choice_parts(choice);
                        if cast_target != leader_key {
                            continue;
                        }
                        if cast_outcome == "A" {
                            a_votes += votes;
                            counts.a_on_leader_votes += *votes as u64;
                            if *voter != leader {
                                counts.rivals_voting_a += 1;
                            }
                        } else {
                            b_votes += votes;
                            counts.b_on_leader_votes += *votes as u64;
                            if *voter != leader {
                                counts.rivals_voting_b += 1;
                                if *votes > 1 {
                                    counts.rivals_bought_extra += 1;
                                }
                            }
                        }
                    }
                    for rival in majors.iter().copied() {
                        if rival == leader || !game.players[rival].alive {
                            continue;
                        }
                        if !resolution.ballots.contains_key(&rival) {
                            counts.rivals_abstaining += 1;
                        }
                        if opening_favor[rival] >= THIRD_VOTE {
                            counts.rivals_could_pay_third += 1;
                        } else if opening_favor[rival] >= SECOND_VOTE {
                            counts.rivals_could_pay_second += 1;
                        }
                    }

                    // What the same table would have decided had every rival
                    // opposed with everything it was holding. `free` is the
                    // weaker claim: one vote each, nobody spends anything.
                    if outcome == "A" && target == leader_key {
                        let mut paid = b_votes;
                        let mut free = b_votes;
                        for rival in majors.iter().copied() {
                            if rival == leader || !game.players[rival].alive {
                                continue;
                            }
                            let cast = resolution
                                .ballots
                                .get(&rival)
                                .map(|(choice, votes)| (choice_parts(choice).0 == "B", *votes));
                            let already = match cast {
                                Some((true, votes)) => votes,
                                _ => 0,
                            };
                            let affordable: u32 = if opening_favor[rival] >= THIRD_VOTE {
                                3
                            } else if opening_favor[rival] >= SECOND_VOTE {
                                2
                            } else {
                                1
                            };
                            paid += affordable.saturating_sub(already);
                            free += 1_u32.saturating_sub(already);
                            // A rival that voted A is switching sides, so its
                            // votes leave the leader's column too.
                            if matches!(cast, Some((false, _))) {
                                let a = resolution.ballots[&rival].1;
                                paid -= a.min(paid);
                                free -= a.min(free);
                            }
                        }
                        let a_without_defectors = a_votes;
                        if paid > a_without_defectors {
                            counts.flip_if_paid += 1;
                        }
                        if free > a_without_defectors {
                            counts.flip_if_free += 1;
                        }
                    }
                }

                for pid in majors.iter().copied() {
                    let moved = game.players[pid].dvp - dvp_prev[pid];
                    counts.residual += moved - attributed.get(&pid).copied().unwrap_or(0);
                }
            }

            // Favor is read the turn a session opens, before anybody has paid
            // for a vote out of it.
            let opening = match (&pending, &observed) {
                (None, Some(_)) => true,
                (Some(prior), Some(current)) => prior.convened != current.convened,
                _ => false,
            };
            if opening {
                opening_favor = game
                    .players
                    .iter()
                    .map(|player| player.diplomatic_favor)
                    .collect();
                // Three candidate targets for the same ballot, read the moment
                // the session opens. `congress_choice` can only express the
                // first of them.
                let living = |pid: &usize| game.players[*pid].alive;
                let diplomatic = majors
                    .iter()
                    .copied()
                    .filter(living)
                    .max_by_key(|pid| (game.players[*pid].dvp, std::cmp::Reverse(*pid)));
                let scoring = majors
                    .iter()
                    .copied()
                    .filter(living)
                    .max_by_key(|pid| (game.score(*pid), std::cmp::Reverse(*pid)));
                let pressing = majors
                    .iter()
                    .copied()
                    .filter(living)
                    .max_by_key(|pid| {
                        (probe.rival_pressure(&game, *pid).1, std::cmp::Reverse(*pid))
                    });
                if let (Some(d), Some(s), Some(p)) = (diplomatic, scoring, pressing) {
                    aim.push((d, s, p));
                }
            }
            pending = observed;
            dvp_prev = game.players.iter().map(|p| p.dvp).collect();
        }

        MapReading {
            winner: game.winner,
            victory_type: game.victory_type.clone().unwrap_or_else(|| "none".into()),
            end_turn,
            peak_era,
            peak_dvp,
            final_dvp: majors.iter().map(|pid| game.players[*pid].dvp).collect(),
            aim,
            penalised,
            counts,
        }
    });

    let mut totals = Counts::default();
    for reading in &readings {
        totals.merge(&reading.counts);
    }
    let decided = readings.iter().filter(|r| r.winner.is_some()).count();

    println!(
        "\ngames: {decided} of {maps} decided ({:.0}%), mean end turn {:.0}",
        100.0 * decided as f64 / maps.max(1) as f64,
        readings.iter().map(|r| r.end_turn as f64).sum::<f64>() / maps.max(1) as f64
    );
    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    for reading in &readings {
        *by_type.entry(reading.victory_type.as_str()).or_default() += 1;
    }
    for (kind, count) in &by_type {
        println!("  {kind:<12} {count}");
    }

    let modern = readings.iter().filter(|r| r.peak_era >= 5).count();
    println!(
        "\nreach: {modern} of {maps} games reach the Modern era ({:.0}%), \
         where world_leader joins every ballot",
        100.0 * modern as f64 / maps.max(1) as f64
    );
    println!(
        "  congress sessions {} ({:.1} per game), scoring resolutions {}, \
         Special Sessions {} (award nothing)",
        totals.sessions,
        totals.sessions as f64 / maps.max(1) as f64,
        totals.resolutions,
        totals.emergencies
    );

    let mut peaks: Vec<i64> = readings.iter().map(|r| r.peak_dvp).collect();
    peaks.sort_unstable();
    let median_peak = peaks.get(peaks.len() / 2).copied().unwrap_or(0);
    let near = peaks.iter().filter(|dvp| **dvp >= 15).count();
    println!(
        "\ndvp: median peak {median_peak} of the 20 a diplomatic victory needs, \
         max {}, {near} of {maps} games reach 15",
        peaks.last().copied().unwrap_or(0)
    );
    let final_mean = readings
        .iter()
        .flat_map(|r| r.final_dvp.iter().copied())
        .sum::<i64>() as f64
        / readings
            .iter()
            .map(|r| r.final_dvp.len())
            .sum::<usize>()
            .max(1) as f64;
    println!("  mean final dvp per major {final_mean:.1}");

    let awarded = totals.from_prediction + totals.from_world_leader + totals.residual;
    println!("\nwhere the points come from ({awarded} awarded, {} denied):", totals.denied_by_world_leader);
    let share = |value: i64| 100.0 * value as f64 / awarded.max(1) as f64;
    println!(
        "  exact prediction   {:>6}  {:>5.1}%   (+1 per voter per resolution)",
        totals.from_prediction,
        share(totals.from_prediction)
    );
    println!(
        "  world_leader A     {:>6}  {:>5.1}%   (+2 to its target)",
        totals.from_world_leader,
        share(totals.from_world_leader)
    );
    println!(
        "  wonders/techs      {:>6}  {:>5.1}%   (residual)",
        totals.residual,
        share(totals.residual)
    );
    println!(
        "  world_leader B     {:>6}          (-2 taken back off the leader)",
        -totals.denied_by_world_leader
    );

    let wl = totals.wl_leader_gained + totals.wl_leader_denied + totals.wl_other_target;
    println!("\nthe world_leader vote ({wl} resolutions, {} tied):", totals.wl_tied);
    println!(
        "  leader gained +2   {:>5}  {:>5.1}%",
        totals.wl_leader_gained,
        100.0 * totals.wl_leader_gained as f64 / wl.max(1) as f64
    );
    println!(
        "  leader denied -2   {:>5}  {:>5.1}%",
        totals.wl_leader_denied,
        100.0 * totals.wl_leader_denied as f64 / wl.max(1) as f64
    );
    println!(
        "  another target     {:>5}  {:>5.1}%",
        totals.wl_other_target,
        100.0 * totals.wl_other_target as f64 / wl.max(1) as f64
    );
    println!(
        "  votes on the leader: {} for A, {} for B",
        totals.a_on_leader_votes, totals.b_on_leader_votes
    );
    let rivals = totals.rivals_voting_a + totals.rivals_voting_b + totals.rivals_abstaining;
    println!(
        "  rival ballots: {} oppose, {} support, {} abstain (of {rivals})",
        totals.rivals_voting_b, totals.rivals_voting_a, totals.rivals_abstaining
    );
    println!(
        "  rivals holding a second vote {} / a third {} — {} ever bought one",
        totals.rivals_could_pay_second, totals.rivals_could_pay_third, totals.rivals_bought_extra
    );
    println!(
        "\ncounterfactual: of {} sessions the leader won, {} flip if its rivals \
         spend the Favor they are holding, {} flip on one vote each",
        totals.wl_leader_gained, totals.flip_if_paid, totals.flip_if_free
    );

    // Every leader-targeting term in `congress_choice` -- world_leader,
    // trade_policy, public_relations -- resolves its target as the empire with
    // the most Diplomatic Victory Points. That is only the right empire to aim
    // a free counter at if it is the empire about to win.
    let mut sessions = 0_usize;
    let mut dvp_is_winner = 0_usize;
    let mut score_is_winner = 0_usize;
    let mut pressure_is_winner = 0_usize;
    let mut dvp_is_score = 0_usize;
    for reading in &readings {
        let Some(winner) = reading.winner else { continue };
        for (diplomatic, scoring, pressing) in &reading.aim {
            sessions += 1;
            dvp_is_winner += usize::from(*diplomatic == winner);
            score_is_winner += usize::from(*scoring == winner);
            pressure_is_winner += usize::from(*pressing == winner);
            dvp_is_score += usize::from(diplomatic == scoring);
        }
    }
    let base = 100.0 / players.max(1) as f64;
    let pct = |value: usize| 100.0 * value as f64 / sessions.max(1) as f64;
    println!(
        "\nwho the congress aims at, over {sessions} sessions of decided games \
         (base rate {base:.1}%):"
    );
    println!(
        "  dvp leader IS the eventual winner       {:>5.1}%   <- the only target congress_choice can name",
        pct(dvp_is_winner)
    );
    println!(
        "  score leader IS the eventual winner     {:>5.1}%",
        pct(score_is_winner)
    );
    println!(
        "  pressure leader IS the eventual winner  {:>5.1}%",
        pct(pressure_is_winner)
    );
    println!(
        "  dvp leader and score leader agree       {:>5.1}%",
        pct(dvp_is_score)
    );

    // The fires-check. A treatment that does not move these numbers is a
    // silent no-op and its eval would be measuring nothing.
    let mut penalties = 0_usize;
    let mut penalties_on_winner = 0_usize;
    for reading in &readings {
        let Some(winner) = reading.winner else { continue };
        for hit in &reading.penalised {
            penalties += 1;
            penalties_on_winner += usize::from(*hit == winner);
        }
    }
    println!("\nfires-check (arm {arm}):");
    println!(
        "  ballots naming this voter's own denial target   {} of {} ({:.1}%)",
        totals.ballots_aimed,
        totals.ballots_cast,
        100.0 * totals.ballots_aimed as f64 / totals.ballots_cast.max(1) as f64
    );
    println!(
        "  ballots carrying a bought vote                  {} ({:.1}%)",
        totals.votes_bought,
        100.0 * totals.votes_bought as f64 / totals.ballots_cast.max(1) as f64
    );
    println!(
        "  targeted penalties that passed                  {} ({:.2} per game)",
        totals.penalties_passed,
        totals.penalties_passed as f64 / maps.max(1) as f64
    );
    println!(
        "  of those, landing on the eventual winner        {penalties_on_winner} of {penalties} ({:.1}%, base {base:.1}%)",
        100.0 * penalties_on_winner as f64 / penalties.max(1) as f64
    );
}
