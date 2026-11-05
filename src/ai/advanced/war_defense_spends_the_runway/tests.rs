use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::ai::advanced::{EmpireCounts, VictoryTarget};
use std::sync::Arc;

/// Two majors with a capital each, met, no units. Seat 0 plays the Science
/// target with a garrison per city on the books, so the first-garrison
/// exception never applies; military power is the observed figure.
fn board(gold: f64, gold_per_turn: f64, ours: f64, theirs: f64, war: bool) -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_931, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for pid in 0..2 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    game.record_contact(0, 1);
    Arc::make_mut(&mut game.observed_military_power).insert(0, ours);
    Arc::make_mut(&mut game.observed_military_power).insert(1, theirs);
    if war {
        game.at_war.insert((0, 1));
    }
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = gold_per_turn;
    game.turn = 161;
    game.current = 0;
    game
}

fn science(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Science);
    if gene {
        ai.enable_war_defense_spends_the_runway();
    }
    ai
}

fn garrisoned() -> EmpireCounts {
    EmpireCounts {
        military: 3,
        ..Default::default()
    }
}

#[test]
fn war_defense_spends_the_runway_is_opt_in() {
    opt_in_off_in_both_controllers("war-defense-spends-the-runway", |ai| {
        ai.war_defense_spends_the_runway
    });
}

/// civvis-20261105T033244Z t161: 168 Gold at -1.7/turn (about 99 turns of
/// runway) under its six-city reserve of 250, power 561 against the Aztecs'
/// 2409. The board holds one city (reserve 125), so the same runway reads
/// 120 Gold at -1.2/turn. Off, the guard takes the queue (the Stock
/// Exchange); on, the war keeps it.
#[test]
fn a_losing_war_with_a_long_runway_keeps_its_queues() {
    let game = board(120.0, -1.2, 561.0, 2409.0, true);
    assert!(science(false).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
    assert!(!science(true).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
}

/// The Rome run the guard was written for: 160 Gold at -24.39/turn is under
/// seven turns of runway (here 120 Gold, five turns), so recovery still comes
/// first.
#[test]
fn a_short_runway_still_recovers() {
    let game = board(120.0, -24.39, 561.0, 2409.0, true);
    assert!(science(true).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
}

/// A rival at parity is not a war we are losing.
#[test]
fn a_war_at_parity_still_recovers() {
    let game = board(120.0, -1.2, 1000.0, 1200.0, true);
    assert!(science(true).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
}

/// A stronger rival we are not at war with does not move the guard.
#[test]
fn peace_still_recovers() {
    let game = board(120.0, -1.2, 561.0, 2409.0, false);
    assert!(science(true).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
}

/// The gene only ever removes a recovery: a solvent seat is not put into one.
#[test]
fn a_solvent_seat_is_untouched() {
    let game = board(400.0, 5.0, 561.0, 2409.0, true);
    assert!(!science(false).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
    assert!(!science(true).live_war_economy_requires_recovery(&game, 0, &garrisoned()));
}
