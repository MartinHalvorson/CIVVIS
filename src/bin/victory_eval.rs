//! End-to-end victory-condition evaluator.
//!
//! Every major in each game is given the same explicit victory target. A run
//! only passes when the real game loop ends with that victory type; no state
//! is injected and no victory check is called directly.
//!
//! ⚠⚠ THIS BINARY WAS DELETED AS UNREFERENCED IN #1278 AND IT WAS NOT
//! UNREFERENCED. The audit looked for callers in the tree and found none, which
//! is true and was the wrong question: everything that cites this tool cites it
//! in prose, and all of it stayed behind.
//!
//! - `docs/EVAL.md` lists `victory_eval --games 2 --players 2` in the battery a
//!   contributor is told to re-run after any AI or rules batch.
//! - `docs/AI_GUIDE.md` gives a full invocation and documents `--target`.
//! - `src/elo.rs` cites its per-target turn limits.
//! - `tools/civ6_civvis_climb.py`'s ★★★★★ note derives the deployed victory
//!   objective from a measurement taken with it.
//!
//! So for eleven days the repository instructed people to run a command that
//! did not exist, and the standing justification for which victory the live
//! agent plays for could not be reproduced or challenged. A binary with no
//! caller in the tree is not the same thing as a binary nothing depends on;
//! the tests below give this one a caller so the audit's question and the real
//! one give the same answer.
//!
//! ## What it measured on restoration (2026-08-17, main `63f3d3c6`)
//!
//! At the live ladder's own profile — `--games 8 --players 6 --turns 250`,
//! seeds 21000000-21000007 — exactly one named victory condition completes
//! inside the budget:
//!
//! | target | completed | winning turns |
//! |---|---|---|
//! | science | 0/8 | — |
//! | culture | 0/8 | — |
//! | religious | **6/8** | 86, 89, 92, 92, 99, 229 |
//! | diplomatic | 0/8 | — |
//! | domination | 0/8 | — |
//! | score | 8/8 | 250 (the clock) |
//!
//! Confirmed on a disjoint stream (seeds 22000000-22000011, same profile):
//! **11/12 religious**, winning turns 82-165. Across both streams the religious
//! lane is 17/20 and has never taken longer than 229 turns.
//!
//! Read with the per-target defaults below, that is not a surprise so much as a
//! restatement of them: this evaluator's own budget for Culture is 1_500 turns
//! and for Science 1_300, and the ladder runs 250. The lane whose budget is
//! nearest the ladder's — Religion at 450, and in practice a third of that — is
//! the only one that lands, and it is the lane the live launchers could not
//! select at all until #1871.
use civvis::ai::{run_game, AdvancedAi, VictoryTarget};
use civvis::game::Game;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn selected_targets(args: &[String]) -> Result<Vec<VictoryTarget>, String> {
    let Some(index) = args.iter().position(|arg| arg == "--target") else {
        return Ok(VictoryTarget::ALL.to_vec());
    };
    let raw = args
        .get(index + 1)
        .ok_or_else(|| "--target requires a value".to_string())?;
    if raw == "all" {
        return Ok(VictoryTarget::ALL.to_vec());
    }
    raw.split(',').map(str::parse).collect()
}

fn default_turn_limit(target: VictoryTarget) -> u32 {
    match target {
        VictoryTarget::Religion => 450,
        VictoryTarget::Domination => 650,
        VictoryTarget::Diplomacy => 750,
        VictoryTarget::Culture => 1_500,
        VictoryTarget::Science => 1_300,
        VictoryTarget::Score => 300,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let targets = selected_targets(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let games = number(&args, "--games", 3);
    let start_seed = number(&args, "--start-seed", 9_000) as u64;
    let players = number(&args, "--players", 2).clamp(2, 8);
    let width = number(&args, "--width", 24).max(16) as i32;
    let height = number(&args, "--height", 16).max(12) as i32;
    let override_turns = args
        .iter()
        .position(|arg| arg == "--turns")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse::<u32>().ok());
    let mut failures = 0;
    let mut winners: BTreeMap<&'static str, BTreeSet<usize>> = BTreeMap::new();
    let started = Instant::now();

    for target in targets.iter().copied() {
        for game_index in 0..games {
            let seed = start_seed + game_index as u64;
            let city_states = if target == VictoryTarget::Diplomacy {
                (players + 1).max(3)
            } else {
                0
            };
            let turns = override_turns.unwrap_or_else(|| default_turn_limit(target));
            let game_started = Instant::now();
            let mut game = Game::new_full(players, width, height, seed, turns, city_states, false);
            let mut ais = AdvancedAi::fleet_targeting(&game, target);
            run_game(&mut game, &mut ais);

            let actual = game.victory_type.as_deref().unwrap_or("none");
            let winner = game.winner.unwrap_or(usize::MAX);
            let major_progress: Vec<(usize, bool, usize, usize, usize)> =
                game.players
                    .iter()
                    .filter(|player| !player.is_minor && !player.is_barbarian)
                    .map(|player| {
                        let era = player
                            .techs
                            .iter()
                            .filter_map(|node| game.rules.techs.get(node).map(|spec| spec.era))
                            .chain(player.civics.iter().filter_map(|node| {
                                game.rules.civics.get(node).map(|spec| spec.era)
                            }))
                            .max()
                            .unwrap_or(0);
                        (
                            player.id,
                            player.alive,
                            era,
                            game.player_city_ids(player.id).len(),
                            player.techs.len(),
                        )
                    })
                    .collect();
            let passed = actual == target.as_str();
            failures += usize::from(!passed);
            if passed {
                winners.entry(target.as_str()).or_default().insert(winner);
            }
            let winner_state = game.players.get(winner);
            let progress = winner_state.map(|player| match target {
                VictoryTarget::Science => format!(
                    "techs={}/{} projects={} distance={:.0} science={:.1}",
                    player.techs.len(),
                    game.rules.techs.len(),
                    player.science_projects.len(),
                    player.exoplanet_distance,
                    game.player_city_ids(winner)
                        .into_iter()
                        .map(|city| game.city_yields(city).science)
                        .sum::<f64>()
                ),
                VictoryTarget::Culture => {
                    let target = game
                        .players
                        .iter()
                        .filter(|rival| {
                            rival.id != winner
                                && rival.alive
                                && !rival.is_minor
                                && !rival.is_barbarian
                        })
                        .map(|rival| game.domestic_tourists(rival.id))
                        .max()
                        .unwrap_or(0);
                    let cities = game.player_city_ids(winner);
                    let theaters = cities
                        .iter()
                        .filter(|city| {
                            game.cities[city].districts.contains_key(civvis::name!("theater_square"))
                                || game.cities[city].districts.contains_key(civvis::name!("acropolis"))
                        })
                        .count();
                    let tourist_improvements = cities
                        .iter()
                        .flat_map(|city| game.cities[city].owned_tiles.iter())
                        .filter_map(|position| game.map.tiles[position].improvement.as_deref())
                        .filter(|improvement| {
                            game.rules.improvements[*improvement]
                                .effects
                                .get("tourism")
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0
                        })
                        .count();
                    format!(
                        "visiting={} target={} domestic={} tourism={:.1}/turn cities={} theaters={} tourist_tiles={} lifetime={:.0}",
                        game.foreign_tourists(winner),
                        target,
                        game.domestic_tourists(winner),
                        game.tourism_per_turn(winner),
                        cities.len(),
                        theaters,
                        tourist_improvements,
                        player.tourism_lifetime,
                    )
                }
                VictoryTarget::Religion => {
                    format!("religion={}", player.religion.as_deref().unwrap_or("none"))
                }
                VictoryTarget::Diplomacy => format!("dvp={}", player.dvp),
                VictoryTarget::Domination => {
                    format!("cities={}", game.player_city_ids(winner).len())
                }
                VictoryTarget::Score => format!("score={}", game.score(winner)),
            });
            println!(
                "{:<11} seed={} target={:<10} actual={:<10} winner={} turn={} world_era={} majors=(id,alive,era,cities,techs){:?} {} [{:.2}s]",
                if passed { "PASS" } else { "FAIL" },
                seed,
                target.as_str(),
                actual,
                if winner == usize::MAX {
                    "none".to_string()
                } else {
                    winner.to_string()
                },
                game.reported_turn(),
                game.world_era,
                major_progress,
                progress.unwrap_or_default(),
                game_started.elapsed().as_secs_f64(),
            );
        }
    }

    println!("\nseat winners by target:");
    for target in &targets {
        println!(
            "  {:<10} {:?}",
            target.as_str(),
            winners.get(target.as_str())
        );
    }
    println!(
        "{} games, {} failures in {:.2}s",
        targets.len() * games,
        failures,
        started.elapsed().as_secs_f64()
    );
    if failures > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    /// ⚠ The reason this test exists is not the parser. #1278 removed this
    /// binary for having "zero tests and zero invocations", and the second half
    /// was false — `docs/EVAL.md` and `docs/AI_GUIDE.md` invoke it, in prose an
    /// audit cannot grep. The first half was true, and it is what made the
    /// second half easy to believe. This is the caller the tree lacked.
    #[test]
    fn the_default_target_set_is_every_victory_the_engine_implements() {
        assert_eq!(selected_targets(&args(&[])).unwrap(), VictoryTarget::ALL);
        assert_eq!(
            selected_targets(&args(&["--target", "all"])).unwrap(),
            VictoryTarget::ALL
        );
    }

    #[test]
    fn a_comma_separated_subset_selects_exactly_those_lanes() {
        assert_eq!(
            selected_targets(&args(&["--target", "religion,score"])).unwrap(),
            vec![VictoryTarget::Religion, VictoryTarget::Score]
        );
        // The aliases `VictoryTarget::from_str` accepts work here too, so a
        // command line copied out of `docs/EVAL.md` runs whichever spelling the
        // prose happened to use.
        assert_eq!(
            selected_targets(&args(&["--target", "religious,diplomatic,conquest"])).unwrap(),
            vec![
                VictoryTarget::Religion,
                VictoryTarget::Diplomacy,
                VictoryTarget::Domination
            ]
        );
    }

    #[test]
    fn an_unknown_target_is_refused_rather_than_silently_dropped() {
        assert!(selected_targets(&args(&["--target", "religous"])).is_err());
        assert!(selected_targets(&args(&["--target"])).is_err());
    }

    /// ★★★★★ THESE NUMBERS ARE THE ARGUMENT, so they are pinned.
    ///
    /// The live ladder runs 250 turns. Four of the six lanes are budgeted here
    /// at more than twice that and two at more than five times it, which is the
    /// cheapest available statement of why a 250-turn ladder attempt aimed at
    /// Science or Culture has never completed one — and it is a statement the
    /// tree lost entirely for eleven days when this file was deleted while
    /// `civ6_civvis_climb.py` went on citing it.
    #[test]
    fn every_lane_declares_the_budget_its_race_actually_needs() {
        let limits: Vec<(&str, u32)> = VictoryTarget::ALL
            .iter()
            .map(|target| (target.as_str(), default_turn_limit(*target)))
            .collect();
        assert_eq!(
            limits,
            vec![
                ("science", 1_300),
                ("culture", 1_500),
                ("religious", 450),
                ("diplomatic", 750),
                ("domination", 650),
                ("score", 300),
            ]
        );
        let ladder_budget = 250;
        assert!(
            limits.iter().all(|(_, turns)| *turns > ladder_budget),
            "every lane's own budget exceeds the ladder's 250 turns; the ladder \
             is not measuring these races at their own length"
        );
    }

    #[test]
    fn numbers_come_off_the_command_line_and_fall_back_when_absent() {
        assert_eq!(number(&args(&["--games", "8"]), "--games", 3), 8);
        assert_eq!(number(&args(&[]), "--games", 3), 3);
        // A flag with an unparseable value takes the default rather than
        // panicking mid-battery.
        assert_eq!(number(&args(&["--games", "lots"]), "--games", 3), 3);
    }
}
