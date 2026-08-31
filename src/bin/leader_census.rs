//! Audit whether victory-denial decisions become responses before a rival wins.
//!
//! The planner has three public diagnostic seams:
//!
//! - [`AdvancedAi::rival_pressure`] exposes the pressure it reads;
//! - [`AdvancedAi::denial_target`] exposes the *actionable* counter it chose;
//! - [`AdvancedAi::denial_is_urgent`] exposes when a terminal clock waives the
//!   normal war-readiness checks.
//!
//! `leader_census` runs ordinary full games and samples those seams after every
//! full turn. It deliberately separates a selected counter from a military
//! response: only a `Conquest` counter is expected to become a declaration of
//! war. Religion, Culture, and Diplomacy are valid non-military counters, so
//! counting their lack of a war as a failed response would answer the wrong
//! question.
//!
//! ```text
//! cargo run --features developer-tools --profile ci --bin leader_census -- \
//!   --players 6 --maps 16 --width 74 --height 46 --city-states 9 \
//!   --turns 400 --seed 940000 --jobs 6
//! ```
//!
//! Diagnostic only: no controller reads this binary or can select it as a
//! strategy.
use civvis::ai::{AdvancedAi, Ai, GrandStrategy, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use std::collections::{BTreeMap, BTreeSet};

/// The generic non-religious pressure bar used by the denial policy.
const DENIAL_BAR: i32 = 78;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Arm {
    Ship,
    InLane,
    StandDown,
    Early,
    EarlyBuild,
}

impl Arm {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ship" => Ok(Self::Ship),
            "in_lane" => Ok(Self::InLane),
            "stand_down" => Ok(Self::StandDown),
            "early" => Ok(Self::Early),
            "early_build" => Ok(Self::EarlyBuild),
            _ => Err("--arm must be ship, in_lane, stand_down, early or early_build".into()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ship => "ship",
            Self::InLane => "in_lane",
            Self::StandDown => "stand_down",
            Self::Early => "early",
            Self::EarlyBuild => "early_build",
        }
    }

    fn configure(self, planner: &mut AdvancedAi) {
        planner.counter_in_lane = matches!(self, Self::InLane | Self::EarlyBuild);
        planner.counter_stand_down = self == Self::StandDown;
        planner.early_score_alarm = matches!(self, Self::Early | Self::EarlyBuild);
    }
}

#[derive(Clone)]
struct Options {
    players: usize,
    maps: usize,
    width: i32,
    height: i32,
    turns: u32,
    seed: u64,
    city_states: usize,
    jobs: usize,
    arm: Arm,
}

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

fn parse_options(args: &[String]) -> Result<Options, String> {
    Ok(Options {
        players: number(args, "--players", 4).max(2),
        maps: number(args, "--maps", 24).max(1),
        width: number(args, "--width", 24).max(8) as i32,
        height: number(args, "--height", 16).max(8) as i32,
        turns: number(args, "--turns", 400).max(1) as u32,
        seed: number(args, "--seed", 900_000) as u64,
        city_states: number(args, "--city-states", 0),
        jobs: number(args, "--jobs", parallel::default_jobs()).max(1),
        arm: Arm::parse(&text(args, "--arm", "ship"))?,
    })
}

#[derive(Clone, Default)]
struct Track {
    first_meter: Option<u32>,
    first_urgent: Option<u32>,
    first_named: Option<u32>,
    first_military_named: Option<u32>,
    first_military_response: Option<u32>,
}

struct MapReading {
    winner: Option<usize>,
    victory_type: String,
    end_turn: u32,
    tracks: BTreeMap<usize, Track>,
    observations: u64,
    denial_selections: u64,
    response_selections: BTreeMap<String, u64>,
    named_pairs: usize,
    military_pairs: usize,
    followed_military_pairs: usize,
}

fn note(slot: &mut Option<u32>, turn: u32) {
    if slot.is_none() {
        *slot = Some(turn);
    }
}

fn median(values: &mut [i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

/// A declaration is the observable follow-through expected from this counter.
/// The other strategies have their own economic, religious, or Congress paths.
fn military_counter(counter: GrandStrategy) -> bool {
    counter == GrandStrategy::Conquest
}

fn major_players(game: &Game) -> Vec<usize> {
    game.players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect()
}

fn run_map(options: &Options, offset: usize) -> MapReading {
    let mut game = Game::new(
        options.players,
        options.width,
        options.height,
        options.seed + offset as u64,
        options.turns,
        options.city_states,
    );
    let weights = Weights::default();
    let mut fleet = AdvancedAi::fleet_weighted(&game, &weights);
    for planner in &mut fleet {
        options.arm.configure(planner);
    }

    let majors = major_players(&game);
    let mut tracks: BTreeMap<usize, Track> =
        majors.iter().map(|pid| (*pid, Track::default())).collect();
    let mut observations = 0_u64;
    let mut denial_selections = 0_u64;
    let mut response_selections = BTreeMap::new();
    let mut named_pairs = BTreeSet::new();
    let mut military_pairs = BTreeSet::new();
    let mut followed_military_pairs = BTreeSet::new();
    let mut end_turn = options.turns;

    for turn in 0..options.turns {
        if game.winner.is_some() {
            end_turn = turn;
            break;
        }
        for (pid, agent) in fleet.iter_mut().enumerate().take(game.players.len()) {
            if game.winner.is_some() {
                break;
            }
            agent.take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        end_turn = turn;

        // Read every live observer's actual configuration. The optional arms
        // change the pressure calculation itself, so a fresh default probe
        // would silently report a different alarm from the one that played.
        for observer in majors.iter().copied() {
            if !game.players[observer].alive {
                continue;
            }
            for target in majors.iter().copied() {
                if target == observer || !game.players[target].alive {
                    continue;
                }
                let (_, pressure) = fleet[observer].rival_pressure(&game, target);
                let track = tracks.get_mut(&target).expect("every major is tracked");
                if pressure >= DENIAL_BAR {
                    note(&mut track.first_meter, turn);
                }
                if fleet[observer].denial_is_urgent(&game, target) {
                    note(&mut track.first_urgent, turn);
                }
            }
        }

        // `denial_target` is deliberately sampled from the observer's own
        // planner: it has already applied visibility and actionability gates.
        for observer in majors.iter().copied() {
            if !game.players[observer].alive {
                continue;
            }
            observations += 1;
            let Some((target, counter)) = fleet[observer].denial_target(&game, observer) else {
                continue;
            };
            denial_selections += 1;
            *response_selections
                .entry(counter.as_str().to_string())
                .or_default() += 1;
            named_pairs.insert((observer, target));
            let track = tracks.get_mut(&target).expect("denial only names majors");
            note(&mut track.first_named, turn);
            if military_counter(counter) {
                military_pairs.insert((observer, target));
                note(&mut track.first_military_named, turn);
            }
        }

        // A Conquest counter is the sole strategy whose ordinary completion
        // condition is a declaration. Do not score Religion/Culture/Diplomacy
        // as failed merely because they never open a war.
        for (observer, target) in military_pairs.iter().copied() {
            if game.is_at_war(observer, target) {
                followed_military_pairs.insert((observer, target));
                let track = tracks.get_mut(&target).expect("every major is tracked");
                note(&mut track.first_military_response, turn);
            }
        }
    }

    MapReading {
        winner: game.winner,
        victory_type: game.victory_type.unwrap_or_else(|| "none".to_string()),
        end_turn,
        tracks,
        observations,
        denial_selections,
        response_selections,
        named_pairs: named_pairs.len(),
        military_pairs: military_pairs.len(),
        followed_military_pairs: followed_military_pairs.len(),
    }
}

fn report_lead_time(label: &str, values: &mut [i64]) {
    match median(values) {
        Some(mid) => println!(
            "  {label:<38} n={:<4} median {mid:>4} turns (min {}, max {})",
            values.len(),
            values.first().copied().unwrap_or(0),
            values.last().copied().unwrap_or(0),
        ),
        None => println!("  {label:<38} n=0"),
    }
}

fn percentage(numerator: usize, denominator: usize) -> f64 {
    100.0 * numerator as f64 / denominator.max(1) as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = parse_options(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    println!(
        "leader_census: {} maps, {}p {}x{}, {} city-states, {} turns, seeds {}..{}, arm {}",
        options.maps,
        options.players,
        options.width,
        options.height,
        options.city_states,
        options.turns,
        options.seed,
        options.seed + options.maps as u64 - 1,
        options.arm.name(),
    );

    let work = options.clone();
    let readings = parallel::map(options.maps, options.jobs, move |offset| {
        run_map(&work, offset)
    });
    let decided: Vec<&MapReading> = readings
        .iter()
        .filter(|reading| reading.winner.is_some())
        .collect();
    let selections: u64 = readings
        .iter()
        .map(|reading| reading.denial_selections)
        .sum();
    let observations: u64 = readings.iter().map(|reading| reading.observations).sum();
    let named_pairs: usize = readings.iter().map(|reading| reading.named_pairs).sum();
    let military_pairs: usize = readings.iter().map(|reading| reading.military_pairs).sum();
    let followed_pairs: usize = readings
        .iter()
        .map(|reading| reading.followed_military_pairs)
        .sum();

    let mut victory_types: BTreeMap<&str, usize> = BTreeMap::new();
    let mut response_selections: BTreeMap<String, u64> = BTreeMap::new();
    for reading in &readings {
        *victory_types.entry(&reading.victory_type).or_default() += 1;
        for (strategy, count) in &reading.response_selections {
            *response_selections.entry(strategy.clone()).or_default() += count;
        }
    }
    println!(
        "\ngames: {} of {} decided ({:.0}%)",
        decided.len(),
        options.maps,
        percentage(decided.len(), options.maps),
    );
    for (victory, count) in victory_types {
        println!("  {victory:<12} {count}");
    }
    println!(
        "\nactionable denial selections: {selections} of {observations} living-major turns ({:.1}%)",
        100.0 * selections as f64 / observations.max(1) as f64,
    );
    if response_selections.is_empty() {
        println!("  response choices: none");
    } else {
        print!("  response choices:");
        for (strategy, count) in response_selections {
            print!(" {strategy}={count}");
        }
        println!();
    }
    println!(
        "  named observer-target pairs: {named_pairs}; military pairs: {military_pairs}; \
         military follow-through: {followed_pairs} of {military_pairs} ({:.0}%)",
        percentage(followed_pairs, military_pairs),
    );
    println!("  non-military selections are reported above, not scored as missing declarations.");

    let mut meter_lead = Vec::new();
    let mut urgent_lead = Vec::new();
    let mut named_lead = Vec::new();
    let mut military_named_lead = Vec::new();
    let mut military_response_lead = Vec::new();
    let mut blind_wins = 0_usize;
    for reading in &decided {
        let winner = reading.winner.expect("decided readings have a winner");
        let track = reading.tracks.get(&winner).expect("winner is a major");
        let end = i64::from(reading.end_turn);
        if let Some(turn) = track.first_meter {
            meter_lead.push(end - i64::from(turn));
        }
        if let Some(turn) = track.first_urgent {
            urgent_lead.push(end - i64::from(turn));
        }
        if let Some(turn) = track.first_named {
            named_lead.push(end - i64::from(turn));
        } else {
            blind_wins += 1;
        }
        if let Some(turn) = track.first_military_named {
            military_named_lead.push(end - i64::from(turn));
        }
        if let Some(turn) = track.first_military_response {
            military_response_lead.push(end - i64::from(turn));
        }
    }
    println!(
        "\nthe eventual winner, over {} decided games:",
        decided.len()
    );
    report_lead_time("any planner pressure ≥78 → win", &mut meter_lead);
    report_lead_time("urgent clock → win", &mut urgent_lead);
    report_lead_time("actionable denial selection → win", &mut named_lead);
    report_lead_time("military denial selection → win", &mut military_named_lead);
    report_lead_time("military follow-through → win", &mut military_response_lead);
    println!(
        "  {:<38} {blind_wins} of {} ({:.0}%)",
        "winner was never selected as a target",
        decided.len(),
        percentage(blind_wins, decided.len()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_documented_arms() {
        assert_eq!(Arm::parse("ship"), Ok(Arm::Ship));
        assert_eq!(Arm::parse("early_build"), Ok(Arm::EarlyBuild));
        assert!(Arm::parse("all_out_war").is_err());
    }

    #[test]
    fn median_is_stable_for_an_unsorted_sample() {
        let mut values = vec![9, 1, 5];
        assert_eq!(median(&mut values), Some(5));
        assert_eq!(values, vec![1, 5, 9]);
    }

    #[test]
    fn only_conquest_is_counted_as_a_missing_war_when_unfinished() {
        assert!(military_counter(GrandStrategy::Conquest));
        for counter in [
            GrandStrategy::Science,
            GrandStrategy::Culture,
            GrandStrategy::Religion,
            GrandStrategy::Diplomacy,
            GrandStrategy::Recovery,
            GrandStrategy::Expansion,
        ] {
            assert!(
                !military_counter(counter),
                "{counter:?} must not be scored as a missing declaration"
            );
        }
    }

    #[test]
    fn tiny_census_observes_each_living_major() {
        let options = Options {
            players: 2,
            maps: 1,
            width: 24,
            height: 16,
            turns: 2,
            seed: 9_400_000,
            city_states: 0,
            jobs: 1,
            arm: Arm::Ship,
        };
        let reading = run_map(&options, 0);
        assert_eq!(reading.tracks.len(), 2);
        assert!(
            reading.observations >= 2,
            "the full-turn sampler saw no majors"
        );
    }
}
