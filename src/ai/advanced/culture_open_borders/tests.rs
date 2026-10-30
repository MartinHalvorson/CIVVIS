use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};
use std::sync::Arc;

/// Four majors who have all met, can all trade passage, and hold Gold; the
/// turn is off the Culture objective's six-turn purchase cadence for seat 0.
fn four_traders(tourism: f64) -> Game {
    let mut game = Game::new_full(4, 30, 16, 79_104, 200, 0, false);
    for rival in 1..4 {
        game.record_contact(0, rival);
    }
    game.turn = 7;
    for pid in 0..4 {
        game.players[pid].gold = 1_000.0;
        game.players[pid]
            .civics
            .insert(crate::name!("early_empire"));
    }
    Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
    game
}

fn culture_seat(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_keeps_every_border_open();
    }
    ai
}

#[test]
fn every_rival_grants_passage_in_one_turn() {
    let mut game = four_traders(60.0);
    culture_seat(true).strategic_bilateral_trade(&mut game, 0, None, GrandStrategy::Expansion);

    for rival in 1..4 {
        assert!(
            game.has_open_borders(0, rival),
            "rival {rival} grants passage"
        );
        assert_eq!(game.international_tourism_multiplier(0, rival, false), 1.25);
    }
}

#[test]
fn off_the_six_turn_cadence_buys_nothing() {
    let mut game = four_traders(60.0);
    culture_seat(false).strategic_bilateral_trade(&mut game, 0, None, GrandStrategy::Expansion);

    for rival in 1..4 {
        assert!(!game.has_open_borders(0, rival), "rival {rival}");
    }
}

#[test]
fn below_the_tourism_floor_the_cadence_is_unchanged() {
    let mut game = four_traders(OPEN_BORDERS_TOURISM_FLOOR - 1.0);
    culture_seat(true).strategic_bilateral_trade(&mut game, 0, None, GrandStrategy::Expansion);

    for rival in 1..4 {
        assert!(!game.has_open_borders(0, rival), "rival {rival}");
    }
}

#[test]
fn a_falling_treasury_keeps_its_reserve() {
    let mut game = four_traders(60.0);
    game.players[0].gold = 30.0;
    game.players[0].gold_per_turn = -4.0;
    let gold = game.players[0].gold;
    let closed = culture_seat(true).keep_every_border_open(
        &mut game,
        0,
        None,
        &BTreeSet::new(),
        &BTreeSet::new(),
    );

    assert_eq!(closed, 0);
    assert_eq!(game.players[0].gold, gold);
}
