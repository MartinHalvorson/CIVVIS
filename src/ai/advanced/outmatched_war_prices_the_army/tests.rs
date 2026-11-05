use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use std::sync::Arc;

/// Two majors with a capital each, met, no units; military power is the
/// observed figure.
fn board(war: bool, ours: f64, theirs: f64) -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_961, 300, 0, false);
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
    game.turn = 168;
    game.current = 0;
    game
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    }
}

fn pricing() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_outmatched_war_prices_the_army();
    ai
}

#[test]
fn outmatched_war_prices_the_army_is_opt_in() {
    opt_in_off_in_both_controllers("outmatched-war-prices-the-army", |ai| {
        ai.outmatched_war_prices_the_army
    });
}

/// 033244Z t168: 577 against 3377 (5.9x, capped at 3), in Recovery, four
/// land bodies short. On, a land unit gains 1050 raw; off, nothing.
#[test]
fn an_outmatched_threatened_war_prices_a_missing_land_unit() {
    let game = board(true, 577.0, 3377.0);
    let recovery = plan(GrandStrategy::Recovery);
    let premium = pricing().outmatched_war_unit_premium(&game, 0, &recovery, true, 4.0);
    assert!((premium - OUTMATCHED_UNIT_PREMIUM * OUTMATCHED_RATIO_CAP).abs() < 1e-9);
    assert_eq!(
        AdvancedAi::new().outmatched_war_unit_premium(&game, 0, &recovery, true, 4.0),
        0.0
    );
}

/// The premium follows the ratio below the cap.
#[test]
fn the_premium_scales_with_the_ratio() {
    let game = board(true, 1000.0, 2000.0);
    let premium =
        pricing().outmatched_war_unit_premium(&game, 0, &plan(GrandStrategy::Recovery), true, 2.0);
    assert!((premium - OUTMATCHED_UNIT_PREMIUM * 2.0).abs() < 1e-9);
}

/// Parity, peace, an unthreatened war, a filled army and a naval unit each
/// leave the price alone.
#[test]
fn only_an_outmatched_threatened_land_gap_is_priced() {
    let recovery = plan(GrandStrategy::Recovery);
    let ai = pricing();
    assert_eq!(
        ai.outmatched_war_unit_premium(&board(true, 1000.0, 1200.0), 0, &recovery, true, 4.0),
        0.0
    );
    assert_eq!(
        ai.outmatched_war_unit_premium(&board(false, 577.0, 3377.0), 0, &recovery, true, 4.0),
        0.0
    );
    assert_eq!(
        ai.outmatched_war_unit_premium(
            &board(true, 577.0, 3377.0),
            0,
            &plan(GrandStrategy::Science),
            true,
            4.0
        ),
        0.0
    );
    let war = board(true, 577.0, 3377.0);
    assert_eq!(
        ai.outmatched_war_unit_premium(&war, 0, &recovery, true, 0.0),
        0.0
    );
    assert_eq!(
        ai.outmatched_war_unit_premium(&war, 0, &recovery, false, 4.0),
        0.0
    );
}
